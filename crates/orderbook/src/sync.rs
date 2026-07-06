//! Binance order-book resync protocol and sequencing.
//!
//! Binance's documented procedure, implemented here exactly:
//!
//! 1. Open the diff stream first and buffer events.
//! 2. Fetch a REST snapshot with `lastUpdateId = L`.
//! 3. Discard buffered events whose final id `u <= L` (already in the snapshot).
//! 4. Apply the first event with `U <= L+1 <= u`.
//! 5. Continue applying; each event's `U` must be `previous u + 1`. Any gap is
//!    fatal for that symbol's book, which is marked stale and excluded from
//!    detection until a fresh snapshot resynchronizes it.
//!
//! A [`SymbolBook`] owns one symbol's ladder and resync state. A [`BookManager`]
//! routes events to the right book and stamps every emitted [`BookUpdate`] with
//! a monotonically increasing sequence number, which is what makes the event
//! log deterministically replayable.

use std::collections::HashMap;

use core_types::{BookStatus, BookUpdate, DepthDelta, DepthSnapshot, PriceLevel, SeqNo, Symbol};

use crate::book::L2Book;

/// Maximum diffs buffered while awaiting a snapshot. A slow snapshot fetch on a
/// busy symbol should not grow memory without bound; the oldest buffered diff is
/// dropped past this cap (and the continuity check will then force another
/// resync if that drop mattered).
const MAX_BUFFER: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Buffering diffs while waiting for a (re)synchronizing snapshot.
    Resyncing,
    /// In sync; diffs are applied directly.
    Live,
}

/// The result of applying one diff against a live book.
enum LiveApply {
    /// Applied and the book advanced.
    Applied,
    /// The diff is entirely older than the book; ignored.
    Old,
    /// A sequence gap: events were missed.
    Gap,
}

/// The top-of-book snapshot captured after a step, used to build a
/// [`BookUpdate`].
struct Top {
    update_id: u64,
    status: BookStatus,
    best_bid: Option<PriceLevel>,
    best_ask: Option<PriceLevel>,
}

/// The outcome of feeding one event to a [`SymbolBook`].
enum Step {
    /// The book advanced (Live).
    Applied(Top),
    /// The event was buffered pending a snapshot.
    Buffered,
    /// The event was older than the book and ignored.
    Ignored,
    /// A gap was detected; the book is now resyncing.
    Gap,
    /// A snapshot was applied; `Top.status` says whether it is Live or still
    /// Stale (a residual gap between snapshot and buffer).
    Resynced(Top),
}

/// One symbol's book plus its resync state machine.
#[derive(Debug, Clone)]
pub struct SymbolBook {
    symbol: Symbol,
    book: L2Book,
    state: State,
    buffer: Vec<DepthDelta>,
}

impl SymbolBook {
    /// A new book for `symbol`, starting in the resyncing state (awaiting its
    /// first snapshot).
    #[must_use]
    pub fn new(symbol: Symbol) -> SymbolBook {
        SymbolBook {
            symbol,
            book: L2Book::new(),
            state: State::Resyncing,
            buffer: Vec::new(),
        }
    }

    /// The symbol this book is for.
    #[must_use]
    pub fn symbol(&self) -> Symbol {
        self.symbol
    }

    /// Read-only access to the price ladder.
    #[must_use]
    pub fn ladder(&self) -> &L2Book {
        &self.book
    }

    /// Current freshness: `Live` when in sync, `Stale` while resyncing.
    #[must_use]
    pub fn status(&self) -> BookStatus {
        match self.state {
            State::Live => BookStatus::Live,
            State::Resyncing => BookStatus::Stale,
        }
    }

    fn top(&self) -> Top {
        Top {
            update_id: self.book.last_update_id(),
            status: self.status(),
            best_bid: self.book.best_bid(),
            best_ask: self.book.best_ask(),
        }
    }

    fn push_buffer(&mut self, diff: DepthDelta) {
        if self.buffer.len() >= MAX_BUFFER {
            self.buffer.remove(0);
        }
        self.buffer.push(diff);
    }

    /// Applies a diff assuming the book is live and contiguous.
    fn try_apply_live(&mut self, diff: &DepthDelta) -> LiveApply {
        let expected = self.book.last_update_id() + 1;
        if diff.final_update_id < expected {
            return LiveApply::Old;
        }
        if diff.first_update_id > expected {
            return LiveApply::Gap;
        }
        // Here U <= expected <= u: contiguous or overlapping, safe to apply.
        self.book.apply_diff_levels(diff);
        LiveApply::Applied
    }

    fn on_diff(&mut self, diff: &DepthDelta) -> Step {
        match self.state {
            State::Resyncing => {
                self.push_buffer(diff.clone());
                Step::Buffered
            }
            State::Live => match self.try_apply_live(diff) {
                LiveApply::Applied => Step::Applied(self.top()),
                LiveApply::Old => Step::Ignored,
                LiveApply::Gap => {
                    // Missed events: drop to resync and buffer this diff for
                    // replay after the next snapshot.
                    self.state = State::Resyncing;
                    self.buffer.clear();
                    self.push_buffer(diff.clone());
                    Step::Gap
                }
            },
        }
    }

    fn on_snapshot(&mut self, snap: &DepthSnapshot) -> Step {
        self.book.load_snapshot(snap);
        self.state = State::Live;

        // Replay buffered diffs in arrival order. Old diffs are skipped; a gap
        // means the snapshot did not overlap the buffer, so re-buffer the
        // remainder and stay stale until another snapshot arrives.
        let buffered = std::mem::take(&mut self.buffer);
        let mut i = 0;
        while i < buffered.len() {
            match self.try_apply_live(&buffered[i]) {
                LiveApply::Applied | LiveApply::Old => i += 1,
                LiveApply::Gap => {
                    self.state = State::Resyncing;
                    self.buffer = buffered[i..].to_vec();
                    break;
                }
            }
        }
        Step::Resynced(self.top())
    }
}

/// Routes market events to per-symbol books and stamps emitted updates with a
/// global monotonic sequence number.
#[derive(Debug, Default)]
pub struct BookManager {
    books: HashMap<Symbol, SymbolBook>,
    next_seq: u64,
}

/// What the manager produced from one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookEvent {
    /// The book advanced; here is the new top of book.
    Updated(BookUpdate),
    /// The diff was buffered pending a snapshot.
    Buffered { symbol: Symbol },
    /// The diff was older than the book and ignored.
    Ignored { symbol: Symbol },
    /// A gap was detected (or a snapshot did not resolve one); the caller must
    /// fetch a fresh REST snapshot for this symbol. The book is stale meanwhile.
    NeedsSnapshot { symbol: Symbol },
}

impl BookManager {
    /// A new, empty manager.
    #[must_use]
    pub fn new() -> BookManager {
        BookManager::default()
    }

    /// Feeds a depth diff. Creates the book (in resyncing state) on first sight
    /// of a symbol.
    pub fn on_diff(&mut self, diff: &DepthDelta) -> BookEvent {
        let symbol = diff.symbol;
        let book = self
            .books
            .entry(symbol)
            .or_insert_with(|| SymbolBook::new(symbol));
        match book.on_diff(diff) {
            Step::Applied(top) => {
                let seq = self.alloc_seq();
                BookEvent::Updated(build_update(symbol, seq, &top, diff.ts))
            }
            Step::Buffered => BookEvent::Buffered { symbol },
            Step::Ignored => BookEvent::Ignored { symbol },
            Step::Gap => BookEvent::NeedsSnapshot { symbol },
            Step::Resynced(_) => unreachable!("on_diff never resyncs"),
        }
    }

    /// Feeds a fresh REST snapshot, replaying any buffered diffs on top of it.
    pub fn on_snapshot(&mut self, snap: &DepthSnapshot) -> BookEvent {
        let symbol = snap.symbol;
        let book = self
            .books
            .entry(symbol)
            .or_insert_with(|| SymbolBook::new(symbol));
        match book.on_snapshot(snap) {
            Step::Resynced(top) => {
                if top.status.is_live() {
                    let seq = self.alloc_seq();
                    BookEvent::Updated(build_update(symbol, seq, &top, snap.ts))
                } else {
                    // A residual gap remains; another snapshot is required.
                    BookEvent::NeedsSnapshot { symbol }
                }
            }
            _ => unreachable!("on_snapshot always resyncs"),
        }
    }

    /// The freshness of a symbol's book, if it exists.
    #[must_use]
    pub fn status(&self, symbol: &Symbol) -> Option<BookStatus> {
        self.books.get(symbol).map(SymbolBook::status)
    }

    /// Read-only access to a symbol's book.
    #[must_use]
    pub fn book(&self, symbol: &Symbol) -> Option<&SymbolBook> {
        self.books.get(symbol)
    }

    /// Number of tracked symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.books.len()
    }

    /// Whether any symbols are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.books.is_empty()
    }

    fn alloc_seq(&mut self) -> SeqNo {
        let seq = SeqNo(self.next_seq);
        self.next_seq += 1;
        seq
    }
}

fn build_update(symbol: Symbol, seq: SeqNo, top: &Top, ts: core_types::Nanos) -> BookUpdate {
    BookUpdate {
        symbol,
        seq,
        update_id: top.update_id,
        status: top.status,
        best_bid: top.best_bid,
        best_ask: top.best_ask,
        ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{Decimal, Nanos};

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn sym() -> Symbol {
        Symbol::from_parts("BTC", "USDT").unwrap()
    }

    fn snap(last: u64, bid: &str, ask: &str) -> DepthSnapshot {
        DepthSnapshot {
            symbol: sym(),
            last_update_id: last,
            bids: vec![PriceLevel::new(d(bid), d("1"))],
            asks: vec![PriceLevel::new(d(ask), d("1"))],
            ts: Nanos(0),
        }
    }

    fn diff(u_first: u64, u_final: u64, bid: &str, ask: &str) -> DepthDelta {
        DepthDelta {
            symbol: sym(),
            first_update_id: u_first,
            final_update_id: u_final,
            bids: vec![PriceLevel::new(d(bid), d("1"))],
            asks: vec![PriceLevel::new(d(ask), d("1"))],
            ts: Nanos(0),
        }
    }

    fn update(ev: &BookEvent) -> &BookUpdate {
        match ev {
            BookEvent::Updated(u) => u,
            other => panic!("expected Updated, got {other:?}"),
        }
    }

    #[test]
    fn snapshot_alone_goes_live() {
        let mut m = BookManager::new();
        let ev = m.on_snapshot(&snap(10, "100", "101"));
        let u = update(&ev);
        assert_eq!(u.status, BookStatus::Live);
        assert_eq!(u.update_id, 10);
        assert_eq!(u.best_bid.unwrap().price, d("100"));
    }

    #[test]
    fn buffers_diffs_before_snapshot_then_replays() {
        let mut m = BookManager::new();
        // Two diffs arrive before the snapshot.
        assert!(matches!(
            m.on_diff(&diff(5, 8, "99", "102")),
            BookEvent::Buffered { .. }
        ));
        assert!(matches!(
            m.on_diff(&diff(9, 12, "100", "101")),
            BookEvent::Buffered { .. }
        ));
        // Snapshot with lastUpdateId 10: the first diff (u=8 <= 10) is discarded,
        // the second (U=9 <= 11 <= u=12) is applied on top.
        let ev = m.on_snapshot(&snap(10, "99.5", "101.5"));
        let u = update(&ev);
        assert_eq!(u.status, BookStatus::Live);
        assert_eq!(u.update_id, 12);
        // The replayed diff moved the book to 100/101.
        assert_eq!(u.best_bid.unwrap().price, d("100"));
        assert_eq!(u.best_ask.unwrap().price, d("101"));
    }

    #[test]
    fn live_continuity_and_old_diffs() {
        let mut m = BookManager::new();
        m.on_snapshot(&snap(10, "100", "101"));
        // An old diff (u=9 <= 10) is ignored.
        assert!(matches!(
            m.on_diff(&diff(8, 9, "50", "60")),
            BookEvent::Ignored { .. }
        ));
        // Contiguous diff U=11 == last+1 is applied.
        let ev = m.on_diff(&diff(11, 13, "100.1", "100.9"));
        assert_eq!(update(&ev).update_id, 13);
        assert!(!m.book(&sym()).unwrap().ladder().is_crossed());
    }

    #[test]
    fn gap_requests_snapshot_and_recovers() {
        let mut m = BookManager::new();
        m.on_snapshot(&snap(10, "100", "101"));
        m.on_diff(&diff(11, 12, "100.1", "100.9"));
        // A gap: next event starts at 20, not 13.
        assert!(matches!(
            m.on_diff(&diff(20, 25, "100.2", "100.8")),
            BookEvent::NeedsSnapshot { .. }
        ));
        assert_eq!(m.status(&sym()), Some(BookStatus::Stale));
        // A fresh snapshot recovers to live.
        let ev = m.on_snapshot(&snap(30, "100.3", "100.7"));
        assert_eq!(update(&ev).status, BookStatus::Live);
        assert_eq!(m.status(&sym()), Some(BookStatus::Live));
    }

    #[test]
    fn sequence_numbers_are_monotonic() {
        let mut m = BookManager::new();
        let mut seqs = Vec::new();
        seqs.push(update(&m.on_snapshot(&snap(10, "100", "101"))).seq);
        seqs.push(update(&m.on_diff(&diff(11, 12, "100.1", "100.9"))).seq);
        seqs.push(update(&m.on_diff(&diff(13, 14, "100.2", "100.8"))).seq);
        for w in seqs.windows(2) {
            assert!(w[1] > w[0], "sequence must strictly increase: {:?}", seqs);
        }
    }
}
