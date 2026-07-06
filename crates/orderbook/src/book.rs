//! The L2 price ladder: a pure, in-memory book with no sequencing logic.
//!
//! Bids and asks are kept in `BTreeMap`s keyed by the raw integer price, which
//! gives ordered iteration for free: the best bid is the largest key and the
//! best ask is the smallest. Applying a level is O(log n); reading the top of
//! book is O(log n) to reach the first/last key. This is correct and simple;
//! if benchmarks later show the top-of-book read matters, a cached best-level
//! can be layered on without changing callers.

use std::collections::BTreeMap;

use core_types::{Decimal, DepthDelta, DepthSnapshot, PriceLevel};

/// A single symbol's L2 book (price ladder only, no resync state).
#[derive(Debug, Clone, Default)]
pub struct L2Book {
    bids: BTreeMap<i128, Decimal>,
    asks: BTreeMap<i128, Decimal>,
    last_update_id: u64,
}

impl L2Book {
    /// An empty book.
    #[must_use]
    pub fn new() -> L2Book {
        L2Book::default()
    }

    /// The exchange update id of the most recently applied event.
    #[must_use]
    pub fn last_update_id(&self) -> u64 {
        self.last_update_id
    }

    /// Replaces the entire book from a REST snapshot. Zero-quantity levels are
    /// dropped defensively (a real snapshot never contains them).
    pub fn load_snapshot(&mut self, snap: &DepthSnapshot) {
        self.bids.clear();
        self.asks.clear();
        for level in &snap.bids {
            if level.qty.is_positive() {
                self.bids.insert(level.price.raw(), level.qty);
            }
        }
        for level in &snap.asks {
            if level.qty.is_positive() {
                self.asks.insert(level.price.raw(), level.qty);
            }
        }
        self.last_update_id = snap.last_update_id;
    }

    /// Applies a diff's level changes and advances `last_update_id`. A level
    /// with zero (or negative) quantity is removed; any other quantity sets the
    /// level. This does no sequence validation; that is the caller's job.
    pub fn apply_diff_levels(&mut self, diff: &DepthDelta) {
        for level in &diff.bids {
            apply_level(&mut self.bids, level);
        }
        for level in &diff.asks {
            apply_level(&mut self.asks, level);
        }
        self.last_update_id = diff.final_update_id;
    }

    /// The best (highest) bid, if any.
    #[must_use]
    pub fn best_bid(&self) -> Option<PriceLevel> {
        self.bids
            .iter()
            .next_back()
            .map(|(&price, &qty)| PriceLevel::new(Decimal::from_raw(price), qty))
    }

    /// The best (lowest) ask, if any.
    #[must_use]
    pub fn best_ask(&self) -> Option<PriceLevel> {
        self.asks
            .iter()
            .next()
            .map(|(&price, &qty)| PriceLevel::new(Decimal::from_raw(price), qty))
    }

    /// The top `n` bid levels, best (highest) first.
    #[must_use]
    pub fn bid_levels(&self, n: usize) -> Vec<PriceLevel> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(&price, &qty)| PriceLevel::new(Decimal::from_raw(price), qty))
            .collect()
    }

    /// The top `n` ask levels, best (lowest) first.
    #[must_use]
    pub fn ask_levels(&self, n: usize) -> Vec<PriceLevel> {
        self.asks
            .iter()
            .take(n)
            .map(|(&price, &qty)| PriceLevel::new(Decimal::from_raw(price), qty))
            .collect()
    }

    /// Number of bid price levels.
    #[must_use]
    pub fn bid_depth(&self) -> usize {
        self.bids.len()
    }

    /// Number of ask price levels.
    #[must_use]
    pub fn ask_depth(&self) -> usize {
        self.asks.len()
    }

    /// Whether the book is crossed (best bid at or above best ask). A healthy
    /// book is never crossed; this is asserted in the property tests.
    #[must_use]
    pub fn is_crossed(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => b.price >= a.price,
            _ => false,
        }
    }
}

fn apply_level(side: &mut BTreeMap<i128, Decimal>, level: &PriceLevel) {
    if level.qty.is_positive() {
        side.insert(level.price.raw(), level.qty);
    } else {
        side.remove(&level.price.raw());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{Nanos, Symbol};

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn sym() -> Symbol {
        Symbol::from_parts("BTC", "USDT").unwrap()
    }

    fn snapshot(bids: &[(&str, &str)], asks: &[(&str, &str)], last: u64) -> DepthSnapshot {
        DepthSnapshot {
            symbol: sym(),
            last_update_id: last,
            bids: bids
                .iter()
                .map(|(p, q)| PriceLevel::new(d(p), d(q)))
                .collect(),
            asks: asks
                .iter()
                .map(|(p, q)| PriceLevel::new(d(p), d(q)))
                .collect(),
            ts: Nanos(0),
        }
    }

    fn diff(
        bids: &[(&str, &str)],
        asks: &[(&str, &str)],
        u_first: u64,
        u_final: u64,
    ) -> DepthDelta {
        DepthDelta {
            symbol: sym(),
            first_update_id: u_first,
            final_update_id: u_final,
            bids: bids
                .iter()
                .map(|(p, q)| PriceLevel::new(d(p), d(q)))
                .collect(),
            asks: asks
                .iter()
                .map(|(p, q)| PriceLevel::new(d(p), d(q)))
                .collect(),
            ts: Nanos(0),
        }
    }

    #[test]
    fn snapshot_sets_top_of_book() {
        let mut b = L2Book::new();
        b.load_snapshot(&snapshot(
            &[("100.00", "1"), ("99.50", "2")],
            &[("100.50", "3"), ("101.00", "4")],
            10,
        ));
        assert_eq!(b.best_bid().unwrap().price, d("100.00"));
        assert_eq!(b.best_ask().unwrap().price, d("100.50"));
        assert_eq!(b.last_update_id(), 10);
        assert!(!b.is_crossed());
    }

    #[test]
    fn diff_updates_and_removes_levels() {
        let mut b = L2Book::new();
        b.load_snapshot(&snapshot(&[("100", "1")], &[("101", "1")], 10));
        // Improve the bid, remove the old ask, add a better ask.
        b.apply_diff_levels(&diff(
            &[("100.25", "2")],
            &[("101", "0"), ("100.75", "5")],
            11,
            12,
        ));
        assert_eq!(b.best_bid().unwrap().price, d("100.25"));
        assert_eq!(b.best_ask().unwrap().price, d("100.75"));
        assert_eq!(b.ask_depth(), 1);
        assert_eq!(b.last_update_id(), 12);
    }

    #[test]
    fn levels_are_ordered() {
        let mut b = L2Book::new();
        b.load_snapshot(&snapshot(
            &[("100", "1"), ("99", "1"), ("98", "1")],
            &[("101", "1"), ("102", "1"), ("103", "1")],
            1,
        ));
        let bids = b.bid_levels(2);
        assert_eq!(bids[0].price, d("100"));
        assert_eq!(bids[1].price, d("99"));
        let asks = b.ask_levels(2);
        assert_eq!(asks[0].price, d("101"));
        assert_eq!(asks[1].price, d("102"));
    }
}
