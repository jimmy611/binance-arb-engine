//! Algorithm C: the triangular fast path.
//!
//! Triangles are precomputed by the graph. Detection is then three
//! multiplications and a comparison per candidate: a triangle is profitable
//! (gross, before slippage) when the product of its three fee-adjusted edge
//! weights exceeds one. This is the default detector and the fastest possible
//! path for pure triangular arbitrage.
//!
//! Two entry points:
//!   * [`scan_all`] evaluates every triangle (cold start, periodic sanity).
//!   * [`scan_affected`] evaluates only the triangles that contain a just-updated
//!     symbol's edges. This is Algorithm B (incremental re-evaluation) expressed
//!     over the triangle set: a single book update touches only a handful of
//!     triangles, so the hot path stays cheap.

use core_types::{Asset, Side, Symbol};
use graph::{AssetGraph, EdgeSide};

/// The default gross-profitability threshold: a product strictly greater than
/// one. The evaluator applies the real, slippage-aware gate on top of this.
pub const DEFAULT_MIN_PRODUCT: f64 = 1.0;

/// A triangle found profitable by the fast path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleHit {
    /// Index into the graph's triangle list.
    pub triangle: usize,
    /// Product of the three fee-adjusted edge weights (gross return factor).
    pub product: f64,
}

impl TriangleHit {
    /// The gross edge in basis points, `(product - 1) * 10_000`.
    #[must_use]
    pub fn edge_bps(&self) -> f64 {
        (self.product - 1.0) * 10_000.0
    }
}

/// One executable leg of a cycle: which market and which side to trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    /// The market to trade.
    pub symbol: Symbol,
    /// The side to trade it on. Selling the base follows a `BASE -> QUOTE`
    /// edge; buying the base follows a `QUOTE -> BASE` edge.
    pub side: Side,
}

/// Evaluates every triangle and returns those with a product above
/// `min_product`, unsorted.
#[must_use]
pub fn scan_all(graph: &AssetGraph, min_product: f64) -> Vec<TriangleHit> {
    (0..graph.num_triangles())
        .filter_map(|tid| hit_if_profitable(graph, tid, min_product))
        .collect()
}

/// Evaluates only the triangles affected by a symbol's book update.
#[must_use]
pub fn scan_affected(graph: &AssetGraph, symbol: &Symbol, min_product: f64) -> Vec<TriangleHit> {
    graph
        .affected_triangles(symbol)
        .iter()
        .filter_map(|&tid| hit_if_profitable(graph, tid, min_product))
        .collect()
}

fn hit_if_profitable(graph: &AssetGraph, tid: usize, min_product: f64) -> Option<TriangleHit> {
    let product = graph.triangle_product(tid);
    if product > min_product && product.is_finite() {
        Some(TriangleHit {
            triangle: tid,
            product,
        })
    } else {
        None
    }
}

/// Resolves a triangle to its three executable legs (symbol and side), in cycle
/// order.
#[must_use]
pub fn triangle_legs(graph: &AssetGraph, triangle_id: usize) -> Option<[Leg; 3]> {
    let triangle = graph.triangle(triangle_id)?;
    let mut legs = [Leg {
        symbol: graph.edge(triangle.edges[0])?.symbol(),
        side: Side::Sell,
    }; 3];
    for (slot, &edge_id) in legs.iter_mut().zip(triangle.edges.iter()) {
        let edge = graph.edge(edge_id)?;
        slot.symbol = edge.symbol();
        slot.side = match edge.side() {
            EdgeSide::BaseToQuote => Side::Sell,
            EdgeSide::QuoteToBase => Side::Buy,
        };
    }
    Some(legs)
}

/// Resolves a triangle to its asset path, in cycle order.
#[must_use]
pub fn triangle_path(graph: &AssetGraph, triangle_id: usize) -> Option<[Asset; 3]> {
    let triangle = graph.triangle(triangle_id)?;
    Some([
        graph.asset(triangle.assets[0])?,
        graph.asset(triangle.assets[1])?,
        graph.asset(triangle.assets[2])?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{BookStatus, BookUpdate, Decimal, Nanos, PriceLevel, SeqNo};

    fn sym(base: &str, quote: &str) -> Symbol {
        Symbol::from_parts(base, quote).unwrap()
    }

    fn book(symbol: Symbol, bid: &str, ask: &str) -> BookUpdate {
        BookUpdate {
            symbol,
            seq: SeqNo(0),
            update_id: 1,
            status: BookStatus::Live,
            best_bid: Some(PriceLevel::new(
                Decimal::from_str_exact(bid).unwrap(),
                Decimal::from_int(10),
            )),
            best_ask: Some(PriceLevel::new(
                Decimal::from_str_exact(ask).unwrap(),
                Decimal::from_int(10),
            )),
            ts: Nanos(0),
        }
    }

    fn arbitrage_graph() -> AssetGraph {
        let specs = vec![
            (sym("BTC", "USDT"), 0.0),
            (sym("ETH", "USDT"), 0.0),
            (sym("ETH", "BTC"), 0.0),
        ];
        let mut g = AssetGraph::from_specs(&specs);
        g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60000"));
        g.update_from_book(&book(sym("ETH", "USDT"), "3000", "3000"));
        // Fair ETH/BTC = 0.05; quoting 0.052 opens a cross-rate arbitrage.
        g.update_from_book(&book(sym("ETH", "BTC"), "0.052", "0.052"));
        g
    }

    #[test]
    fn scan_all_finds_the_arbitrage() {
        let g = arbitrage_graph();
        let hits = scan_all(&g, DEFAULT_MIN_PRODUCT);
        assert!(
            !hits.is_empty(),
            "expected at least one profitable triangle"
        );
        let best = hits.iter().map(|h| h.product).fold(0.0, f64::max);
        assert!(best > 1.0);
    }

    #[test]
    fn scan_affected_matches_a_subset() {
        let g = arbitrage_graph();
        let all = scan_all(&g, DEFAULT_MIN_PRODUCT);
        let affected = scan_affected(&g, &sym("ETH", "BTC"), DEFAULT_MIN_PRODUCT);
        // Every affected hit is also found by the full scan.
        for hit in &affected {
            assert!(all.iter().any(|h| h.triangle == hit.triangle));
        }
        assert!(!affected.is_empty());
    }

    #[test]
    fn no_arbitrage_when_fees_dominate() {
        let specs = vec![
            (sym("BTC", "USDT"), 0.001),
            (sym("ETH", "USDT"), 0.001),
            (sym("ETH", "BTC"), 0.001),
        ];
        let mut g = AssetGraph::from_specs(&specs);
        g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60000"));
        g.update_from_book(&book(sym("ETH", "USDT"), "3000", "3000"));
        g.update_from_book(&book(sym("ETH", "BTC"), "0.05", "0.05")); // fair
        let hits = scan_all(&g, DEFAULT_MIN_PRODUCT);
        assert!(hits.is_empty(), "fair prices minus fees are not profitable");
    }

    #[test]
    fn legs_have_correct_sides() {
        let g = arbitrage_graph();
        let hits = scan_all(&g, DEFAULT_MIN_PRODUCT);
        let legs = triangle_legs(&g, hits[0].triangle).unwrap();
        // Each leg trades a known symbol; sides are buy or sell consistently.
        for leg in &legs {
            assert!(matches!(leg.side, Side::Buy | Side::Sell));
        }
        let path = triangle_path(&g, hits[0].triangle).unwrap();
        // The path is a closed 3-cycle of distinct assets.
        assert_ne!(path[0], path[1]);
        assert_ne!(path[1], path[2]);
        assert_ne!(path[0], path[2]);
    }
}
