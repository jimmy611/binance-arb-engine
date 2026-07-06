//! The engine pipeline: wires market events through the book, graph, detector,
//! evaluator, and risk manager, in dry-run.
//!
//! This is the single-threaded hot path. A background task feeds
//! [`core_types::MarketEvent`]s in over a channel; each event is logged to the
//! WAL, applied to the order book, and, when a book advances, used to update the
//! currency graph and re-scan only the affected triangles. Profitable triangles
//! are sized with the VWAP optimizer, checked against risk, and (in dry-run)
//! logged and published to the streaming hub. Nothing is ever submitted here.

use std::collections::HashMap;
use std::sync::Arc;

use control_api::hub::StreamEvent;
use control_api::{pb, ControlState, Hub};
use core_types::{BookStatus, BookUpdate, Decimal, MarketEvent, Nanos, PriceLevel, Side, Symbol};
use detector::{scan_affected, triangle_legs};
use evaluator::{optimize_cycle, LegBook};
use graph::AssetGraph;
use orderbook::{BookEvent, BookManager};
use persistence::{WalPayload, WalWriter};
use risk::RiskManager;

const DEPTH_LEVELS: usize = 25;
const MIN_PRODUCT: f64 = 1.0;
const PERSISTENCE_REQUIRED: u32 = 2;

/// The mutable, single-threaded engine state.
pub struct Pipeline {
    graph: AssetGraph,
    books: BookManager,
    risk: RiskManager,
    wal: WalWriter,
    hub: Arc<Hub>,
    state: Arc<ControlState>,
    fee: f64,
    opportunities: u64,
    persistence: HashMap<usize, u32>,
    book_updates: u64,
}

impl Pipeline {
    /// Builds a pipeline over the given components.
    pub fn new(
        graph: AssetGraph,
        risk: RiskManager,
        wal: WalWriter,
        hub: Arc<Hub>,
        state: Arc<ControlState>,
        fee: f64,
    ) -> Pipeline {
        Pipeline {
            graph,
            books: BookManager::new(),
            risk,
            wal,
            hub,
            state,
            fee,
            opportunities: 0,
            persistence: HashMap::new(),
            book_updates: 0,
        }
    }

    /// Number of opportunities detected so far.
    #[must_use]
    pub fn opportunities(&self) -> u64 {
        self.opportunities
    }

    /// Number of book updates applied so far.
    #[must_use]
    pub fn book_updates(&self) -> u64 {
        self.book_updates
    }

    /// Handles one market event. Returns a symbol whose book needs a fresh REST
    /// snapshot, if any (the caller fetches it asynchronously).
    pub fn handle(&mut self, event: MarketEvent) -> Option<Symbol> {
        let ts = event.ts();
        let _ = self.wal.append(ts, WalPayload::Market(event.clone()));

        match event {
            MarketEvent::DepthDelta(diff) => match self.books.on_diff(&diff) {
                BookEvent::Updated(bu) => {
                    self.on_book_update(bu);
                    None
                }
                BookEvent::NeedsSnapshot { symbol } => Some(symbol),
                BookEvent::Buffered { .. } | BookEvent::Ignored { .. } => None,
            },
            MarketEvent::DepthSnapshot(snap) => {
                if let BookEvent::Updated(bu) = self.books.on_snapshot(&snap) {
                    self.on_book_update(bu);
                }
                None
            }
            // Book tickers and trades are not used to drive detection (the depth
            // book is authoritative); trades feed empirical models later.
            MarketEvent::BookTicker(_) | MarketEvent::Trade(_) => None,
        }
    }

    fn on_book_update(&mut self, bu: BookUpdate) {
        self.book_updates += 1;
        self.graph.update_from_book(&bu);
        let _ = self.wal.append(bu.ts, WalPayload::Book(bu));
        self.publish_ticker(&bu);

        if !bu.status.is_live() {
            return;
        }

        // Re-evaluate only the triangles this symbol participates in.
        let hits = scan_affected(&self.graph, &bu.symbol, MIN_PRODUCT);
        let affected: Vec<usize> = self.graph.affected_triangles(&bu.symbol).to_vec();
        let profitable: std::collections::HashSet<usize> =
            hits.iter().map(|h| h.triangle).collect();

        for tid in affected {
            if profitable.contains(&tid) {
                let count = self.persistence.entry(tid).or_insert(0);
                *count += 1;
                let persisted = *count;
                if let Some(hit) = hits.iter().find(|h| h.triangle == tid) {
                    self.evaluate(tid, hit.product, persisted);
                }
            } else {
                self.persistence.insert(tid, 0);
            }
        }
    }

    fn evaluate(&mut self, tid: usize, product: f64, persisted: u32) {
        let Some(legs) = triangle_legs(&self.graph, tid) else {
            return;
        };
        let Some(leg_books) = self.leg_books(&legs) else {
            return;
        };

        let shape = optimize_cycle(&leg_books[0], &leg_books[1], &leg_books[2]);
        let size = shape.optimal_size();
        let profit = shape.optimal_profit();
        if size <= 0.0 || profit <= 0.0 {
            return;
        }

        let edge_bps = (product - 1.0) * 10_000.0;
        let start_asset = self
            .graph
            .triangle(tid)
            .and_then(|t| self.graph.asset(t.assets[0]));
        let Some(start_asset) = start_asset else {
            return;
        };

        // Risk check (notional approximated in starting-asset units).
        let notional = Decimal::from_f64(size).unwrap_or(Decimal::ZERO);
        let now_ms = Nanos::now().as_millis();
        let decision = self.risk.check(notional, start_asset, 0, now_ms);

        let status = if !decision.is_approved() {
            "filtered"
        } else if persisted >= PERSISTENCE_REQUIRED {
            "live"
        } else {
            "ghost"
        };

        self.opportunities += 1;
        let confidence = f64::from(persisted.min(3)) / 3.0 * 0.9 + 0.05;
        let opp = self.build_opportunity(
            tid, &legs, product, edge_bps, profit, size, confidence, status,
        );

        if decision.is_approved() && persisted >= PERSISTENCE_REQUIRED {
            tracing::info!(
                cycle = %opp.id,
                edge_bps = format!("{edge_bps:.1}"),
                size = format!("{size:.2}"),
                profit = format!("{profit:.4}"),
                "dry-run: opportunity accepted (no order sent)"
            );
        }

        self.state.record_opportunity(opp.clone());
        self.hub.publish(StreamEvent::Opportunity(opp));
    }

    /// Builds the per-leg consumable books from the current ladders.
    fn leg_books(&self, legs: &[detector::Leg; 3]) -> Option<[LegBook; 3]> {
        let mut out: Vec<LegBook> = Vec::with_capacity(3);
        for leg in legs {
            let book = self.books.book(&leg.symbol)?;
            let ladder = book.ladder();
            let levels: Vec<PriceLevel> = match leg.side {
                Side::Sell => ladder.bid_levels(DEPTH_LEVELS),
                Side::Buy => ladder.ask_levels(DEPTH_LEVELS),
            };
            if levels.is_empty() {
                return None;
            }
            out.push(LegBook {
                side: leg.side,
                fee: self.fee,
                levels,
            });
        }
        let [a, b, c]: [LegBook; 3] = out.try_into().ok()?;
        Some([a, b, c])
    }

    #[allow(clippy::too_many_arguments)]
    fn build_opportunity(
        &self,
        tid: usize,
        legs: &[detector::Leg; 3],
        product: f64,
        edge_bps: f64,
        profit: f64,
        size: f64,
        confidence: f64,
        status: &str,
    ) -> pb::Opportunity {
        let path = self
            .graph
            .triangle(tid)
            .map(|t| {
                let mut names: Vec<String> = t
                    .assets
                    .iter()
                    .filter_map(|&v| self.graph.asset(v).map(|a| a.as_str().to_owned()))
                    .collect();
                if let Some(first) = names.first().cloned() {
                    names.push(first);
                }
                names
            })
            .unwrap_or_default();

        let pb_legs = legs
            .iter()
            .map(|leg| pb::OpportunityLeg {
                symbol: Some(pb_symbol(&leg.symbol)),
                side: pb_side(leg.side) as i32,
                price: None,
                qty: None,
            })
            .collect();

        pb::Opportunity {
            id: format!("opp-{}", self.opportunities),
            path,
            legs: pb_legs,
            gross_product: product,
            edge_bps,
            ev_quote: profit,
            size_quote: size,
            confidence,
            mode: "dry_run".to_owned(),
            status: status.to_owned(),
            ts: Some(pb_now()),
        }
    }

    fn publish_ticker(&self, bu: &BookUpdate) {
        let top = pb::BookTop {
            symbol: Some(pb_symbol(&bu.symbol)),
            bid_price: bu.best_bid.map(|l| pb_decimal(l.price)),
            bid_qty: bu.best_bid.map(|l| pb_decimal(l.qty)),
            ask_price: bu.best_ask.map(|l| pb_decimal(l.price)),
            ask_qty: bu.best_ask.map(|l| pb_decimal(l.qty)),
            status: if bu.status == BookStatus::Live {
                "live"
            } else {
                "stale"
            }
            .to_owned(),
            ts: Some(pb_now()),
        };
        self.hub.publish(StreamEvent::Ticker(top));
    }
}

fn pb_symbol(symbol: &Symbol) -> pb::Symbol {
    pb::Symbol {
        raw: symbol.as_str().to_owned(),
        base: symbol.base().as_str().to_owned(),
        quote: symbol.quote().as_str().to_owned(),
    }
}

fn pb_side(side: Side) -> pb::Side {
    match side {
        Side::Buy => pb::Side::Buy,
        Side::Sell => pb::Side::Sell,
    }
}

fn pb_decimal(d: Decimal) -> pb::Decimal {
    pb::Decimal {
        value: d.to_string(),
    }
}

fn pb_now() -> pb::Timestamp {
    pb::Timestamp {
        unix_nanos: Nanos::now().get(),
    }
}
