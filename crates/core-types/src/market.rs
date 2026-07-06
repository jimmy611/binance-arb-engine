//! The unified market-data vocabulary.
//!
//! Every venue adapter normalizes its exchange-specific payloads into these
//! types, and everything downstream (order book, graph, detector) speaks only
//! this vocabulary. Adding an exchange therefore never touches the engine core:
//! it only adds a new normalizer that produces [`MarketEvent`]s.

use serde::{Deserialize, Serialize};

use crate::decimal::Decimal;
use crate::symbol::Symbol;
use crate::time::Nanos;

/// Order side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Buy the base asset (lift the ask).
    Buy,
    /// Sell the base asset (hit the bid).
    Sell,
}

impl Side {
    /// The opposite side.
    #[inline]
    #[must_use]
    pub const fn opposite(self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

/// One level of an order book: a price and the quantity resting at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price of the level.
    pub price: Decimal,
    /// Quantity resting at the level. A quantity of zero in a depth diff means
    /// "remove this level".
    pub qty: Decimal,
}

impl PriceLevel {
    /// Constructs a level.
    #[inline]
    #[must_use]
    pub const fn new(price: Decimal, qty: Decimal) -> PriceLevel {
        PriceLevel { price, qty }
    }

    /// The notional value of the level (`price * qty`), or `None` on overflow.
    #[inline]
    #[must_use]
    pub fn notional(self) -> Option<Decimal> {
        self.price.checked_mul(self.qty)
    }
}

/// Best bid and best ask for a symbol, from Binance's `@bookTicker` stream.
/// This is the lowest-latency top-of-book signal and, being all-`Copy`, is
/// cheap to pass by value on the hot path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookTicker {
    /// The symbol this quote is for.
    pub symbol: Symbol,
    /// Exchange update id for this quote (Binance `u`). Monotonic per symbol.
    pub update_id: u64,
    /// Best bid price.
    pub bid_price: Decimal,
    /// Quantity at the best bid.
    pub bid_qty: Decimal,
    /// Best ask price.
    pub ask_price: Decimal,
    /// Quantity at the best ask.
    pub ask_qty: Decimal,
    /// Wall-clock time the event was received/normalized.
    pub ts: Nanos,
}

/// A full depth snapshot from the REST `/api/v3/depth` endpoint, used to seed a
/// book before applying the diff stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepthSnapshot {
    /// The symbol.
    pub symbol: Symbol,
    /// Binance `lastUpdateId`: diffs with a final update id at or before this
    /// are already reflected in the snapshot and must be discarded.
    pub last_update_id: u64,
    /// Bid levels, exchange order (descending price).
    pub bids: Vec<PriceLevel>,
    /// Ask levels, exchange order (ascending price).
    pub asks: Vec<PriceLevel>,
    /// Wall-clock time the snapshot was received.
    pub ts: Nanos,
}

/// A depth diff from the `@depth` stream. Applying it requires the exact
/// sequence-continuity check `U <= last_applied + 1 <= u`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepthDelta {
    /// The symbol.
    pub symbol: Symbol,
    /// First update id covered by this event (Binance `U`).
    pub first_update_id: u64,
    /// Final update id covered by this event (Binance `u`).
    pub final_update_id: u64,
    /// Changed bid levels (a zero quantity removes the level).
    pub bids: Vec<PriceLevel>,
    /// Changed ask levels (a zero quantity removes the level).
    pub asks: Vec<PriceLevel>,
    /// Wall-clock time the event was received.
    pub ts: Nanos,
}

/// A public trade print from the `@trade` stream. Used by the evaluator's
/// empirical models (opportunity-lifetime and fill-slippage distributions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    /// The symbol.
    pub symbol: Symbol,
    /// Exchange trade id.
    pub trade_id: u64,
    /// Execution price.
    pub price: Decimal,
    /// Executed quantity.
    pub qty: Decimal,
    /// `true` if the buyer was the maker (the aggressor was a seller).
    pub buyer_is_maker: bool,
    /// Exchange event time.
    pub ts: Nanos,
}

/// The unified market event produced by every adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarketEvent {
    /// Best bid/ask update (lowest latency top-of-book).
    BookTicker(BookTicker),
    /// Full REST depth snapshot used to seed or resync a book.
    DepthSnapshot(DepthSnapshot),
    /// Incremental depth diff.
    DepthDelta(DepthDelta),
    /// Public trade print.
    Trade(Trade),
}

impl MarketEvent {
    /// The symbol this event pertains to.
    #[inline]
    #[must_use]
    pub fn symbol(&self) -> Symbol {
        match self {
            MarketEvent::BookTicker(e) => e.symbol,
            MarketEvent::DepthSnapshot(e) => e.symbol,
            MarketEvent::DepthDelta(e) => e.symbol,
            MarketEvent::Trade(e) => e.symbol,
        }
    }

    /// The wall-clock receive time of this event.
    #[inline]
    #[must_use]
    pub fn ts(&self) -> Nanos {
        match self {
            MarketEvent::BookTicker(e) => e.ts,
            MarketEvent::DepthSnapshot(e) => e.ts,
            MarketEvent::DepthDelta(e) => e.ts,
            MarketEvent::Trade(e) => e.ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn sym() -> Symbol {
        Symbol::from_parts("BTC", "USDT").unwrap()
    }

    #[test]
    fn side_opposite() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }

    #[test]
    fn level_notional() {
        let l = PriceLevel::new(d("100"), d("2.5"));
        assert_eq!(l.notional().unwrap(), d("250"));
    }

    #[test]
    fn event_accessors() {
        let ev = MarketEvent::BookTicker(BookTicker {
            symbol: sym(),
            update_id: 7,
            bid_price: d("100"),
            bid_qty: d("1"),
            ask_price: d("100.01"),
            ask_qty: d("2"),
            ts: Nanos(42),
        });
        assert_eq!(ev.symbol(), sym());
        assert_eq!(ev.ts(), Nanos(42));
    }

    #[test]
    fn market_event_serde_roundtrip() {
        let ev = MarketEvent::Trade(Trade {
            symbol: sym(),
            trade_id: 1,
            price: d("100"),
            qty: d("0.5"),
            buyer_is_maker: true,
            ts: Nanos(1),
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: MarketEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
    }
}
