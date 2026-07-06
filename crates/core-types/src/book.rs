//! Shared order-book output types.
//!
//! The live, mutable L2 book and the Binance resync state machine live in the
//! `orderbook` crate (built at M3). What lives here is the *contract* the book
//! emits: a [`BookUpdate`] carrying the engine sequence number, the freshness
//! status, and the current top of book. The graph and detector consume only
//! this, so they never depend on the book's internal representation.

use serde::{Deserialize, Serialize};

use crate::decimal::Decimal;
use crate::ids::SeqNo;
use crate::market::PriceLevel;
use crate::symbol::Symbol;
use crate::time::Nanos;

/// Whether a symbol's book is currently trustworthy.
///
/// A sequence gap in the diff stream makes the book's state uncertain. Per the
/// Binance protocol that symbol is marked [`BookStatus::Stale`] and excluded
/// from arbitrage detection until a fresh snapshot resynchronizes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookStatus {
    /// The book is in sync and safe to trade against.
    Live,
    /// The book has a detected gap and is being resynchronized. Excluded from
    /// detection.
    Stale,
}

impl BookStatus {
    /// Returns `true` if the book may be used for detection.
    #[inline]
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, BookStatus::Live)
    }
}

/// A snapshot of the top of a symbol's book, emitted after every applied update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookUpdate {
    /// The symbol.
    pub symbol: Symbol,
    /// Engine-assigned monotonic sequence number for deterministic replay.
    pub seq: SeqNo,
    /// The exchange update id that produced this state (Binance `u`).
    pub update_id: u64,
    /// Freshness of the book.
    pub status: BookStatus,
    /// Best bid, if the bid side is non-empty.
    pub best_bid: Option<PriceLevel>,
    /// Best ask, if the ask side is non-empty.
    pub best_ask: Option<PriceLevel>,
    /// Wall-clock time of the update.
    pub ts: Nanos,
}

impl BookUpdate {
    /// The mid price `(bid + ask) / 2`, if both sides are present. Returns
    /// `None` if either side is empty or the arithmetic overflows.
    #[must_use]
    pub fn mid_price(&self) -> Option<Decimal> {
        let bid = self.best_bid?.price;
        let ask = self.best_ask?.price;
        bid.checked_add(ask)?.checked_div(Decimal::from_int(2))
    }

    /// The absolute spread `ask - bid`, if both sides are present.
    #[must_use]
    pub fn spread(&self) -> Option<Decimal> {
        let bid = self.best_bid?.price;
        let ask = self.best_ask?.price;
        ask.checked_sub(bid)
    }

    /// Returns `true` if the book is crossed (best bid at or above best ask),
    /// which must never happen on a healthy book and is asserted against in the
    /// order-book property tests.
    #[must_use]
    pub fn is_crossed(&self) -> bool {
        match (self.best_bid, self.best_ask) {
            (Some(b), Some(a)) => b.price >= a.price,
            _ => false,
        }
    }

    /// Whether this update is usable for detection: live and two-sided.
    #[must_use]
    pub fn is_tradeable(&self) -> bool {
        self.status.is_live() && self.best_bid.is_some() && self.best_ask.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn update(bid: &str, ask: &str) -> BookUpdate {
        BookUpdate {
            symbol: Symbol::from_parts("BTC", "USDT").unwrap(),
            seq: SeqNo(1),
            update_id: 100,
            status: BookStatus::Live,
            best_bid: Some(PriceLevel::new(d(bid), d("1"))),
            best_ask: Some(PriceLevel::new(d(ask), d("1"))),
            ts: Nanos(0),
        }
    }

    #[test]
    fn mid_and_spread() {
        let u = update("100", "102");
        assert_eq!(u.mid_price().unwrap(), d("101"));
        assert_eq!(u.spread().unwrap(), d("2"));
    }

    #[test]
    fn crossed_detection() {
        assert!(!update("100", "102").is_crossed());
        assert!(update("103", "102").is_crossed());
    }

    #[test]
    fn tradeable_requires_live_and_two_sided() {
        let mut u = update("100", "102");
        assert!(u.is_tradeable());
        u.status = BookStatus::Stale;
        assert!(!u.is_tradeable());
        u.status = BookStatus::Live;
        u.best_ask = None;
        assert!(!u.is_tradeable());
    }
}
