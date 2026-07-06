//! Property-based tests for the order-book invariants the engine relies on:
//! the book is never crossed, applied diffs advance the update id, and every
//! emitted update carries a strictly increasing sequence number.

use core_types::{Decimal, DepthDelta, DepthSnapshot, Nanos, PriceLevel, Symbol};
use orderbook::{BookEvent, BookManager, L2Book};
use proptest::prelude::*;

fn sym() -> Symbol {
    Symbol::from_parts("BTC", "USDT").unwrap()
}

fn level(price_raw: i128, qty_raw: i128) -> PriceLevel {
    PriceLevel::new(Decimal::from_raw(price_raw), Decimal::from_raw(qty_raw))
}

fn diff(u_first: u64, u_final: u64, bids: Vec<PriceLevel>, asks: Vec<PriceLevel>) -> DepthDelta {
    DepthDelta {
        symbol: sym(),
        first_update_id: u_first,
        final_update_id: u_final,
        bids,
        asks,
        ts: Nanos(0),
    }
}

proptest! {
    /// With bids drawn from a low price band and asks from a disjoint high band,
    /// the book can never be crossed no matter how snapshots and diffs interleave.
    #[test]
    fn book_never_crosses(
        snap_bids in prop::collection::vec((1i128..900_000, 1i128..1_000_000), 0..16),
        snap_asks in prop::collection::vec((1_100_000i128..2_000_000, 1i128..1_000_000), 0..16),
        bid_updates in prop::collection::vec((1i128..900_000, 0i128..1_000_000), 0..40),
        ask_updates in prop::collection::vec((1_100_000i128..2_000_000, 0i128..1_000_000), 0..40),
    ) {
        let mut book = L2Book::new();
        book.load_snapshot(&DepthSnapshot {
            symbol: sym(),
            last_update_id: 1,
            bids: snap_bids.iter().map(|(p, q)| level(*p, *q)).collect(),
            asks: snap_asks.iter().map(|(p, q)| level(*p, *q)).collect(),
            ts: Nanos(0),
        });
        prop_assert!(!book.is_crossed());

        let mut id = 2u64;
        for (p, q) in &bid_updates {
            book.apply_diff_levels(&diff(id, id, vec![level(*p, *q)], vec![]));
            prop_assert!(!book.is_crossed(), "crossed after bid update");
            id += 1;
        }
        for (p, q) in &ask_updates {
            book.apply_diff_levels(&diff(id, id, vec![], vec![level(*p, *q)]));
            prop_assert!(!book.is_crossed(), "crossed after ask update");
            id += 1;
        }
    }

    /// A run of contiguous diffs is always applied, the update id tracks the
    /// diff's final id, and emitted sequence numbers strictly increase.
    #[test]
    fn contiguous_diffs_advance_and_seq_is_monotonic(steps in 1usize..60) {
        let mut m = BookManager::new();
        let snap = DepthSnapshot {
            symbol: sym(),
            last_update_id: 100,
            bids: vec![level(500_000, 100)],
            asks: vec![level(1_500_000, 100)],
            ts: Nanos(0),
        };
        let mut last_seq = match m.on_snapshot(&snap) {
            BookEvent::Updated(u) => {
                prop_assert_eq!(u.update_id, 100);
                u.seq
            }
            other => return Err(TestCaseError::fail(format!("snapshot: {other:?}"))),
        };

        for expected in (101u64..).take(steps) {
            let ev = m.on_diff(&diff(expected, expected, vec![level(500_000, 100)], vec![]));
            match ev {
                BookEvent::Updated(u) => {
                    prop_assert_eq!(u.update_id, expected);
                    prop_assert!(u.seq > last_seq, "seq must increase");
                    last_seq = u.seq;
                }
                other => return Err(TestCaseError::fail(format!("diff: {other:?}"))),
            }
        }
    }

    /// Any gap (a diff whose first id is beyond the expected next id) forces a
    /// resync request and marks the book stale.
    #[test]
    fn gaps_are_detected(gap in 2u64..1_000) {
        let mut m = BookManager::new();
        m.on_snapshot(&DepthSnapshot {
            symbol: sym(),
            last_update_id: 100,
            bids: vec![level(500_000, 100)],
            asks: vec![level(1_500_000, 100)],
            ts: Nanos(0),
        });
        // Expected next is 101; start at 101 + gap to force a hole.
        let start = 101 + gap;
        let ev = m.on_diff(&diff(start, start, vec![level(500_000, 100)], vec![]));
        let needs_snapshot = matches!(ev, BookEvent::NeedsSnapshot { .. });
        prop_assert!(needs_snapshot, "expected NeedsSnapshot, got {ev:?}");
    }
}
