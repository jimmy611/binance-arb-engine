//! Hot state: the low-latency cache of current top-of-book and engine mode.
//!
//! In production this is Redis (shared across processes and the control API); in
//! development and tests it is an in-memory map. Both implement [`HotState`], so
//! the engine depends on the trait and the Redis client swaps in when that infra
//! is available (see `docker-compose.yml`). The Redis implementation is deferred
//! until the service can be exercised; it is not stubbed with fake behavior.

use std::collections::HashMap;
use std::sync::Mutex;

use core_types::{BookUpdate, EngineMode, Symbol};

/// The hot-state interface the engine and control API depend on.
pub trait HotState: Send + Sync {
    /// Records the latest top of book for a symbol.
    fn update_top(&self, update: BookUpdate);
    /// The latest top of book for a symbol, if known.
    fn top(&self, symbol: &Symbol) -> Option<BookUpdate>;
    /// Sets the current engine mode.
    fn set_mode(&self, mode: EngineMode);
    /// The current engine mode.
    fn mode(&self) -> EngineMode;
}

#[derive(Default)]
struct Inner {
    tops: HashMap<Symbol, BookUpdate>,
    mode: EngineMode,
}

/// An in-memory [`HotState`] for development and tests.
#[derive(Default)]
pub struct InMemoryHotState {
    inner: Mutex<Inner>,
}

impl InMemoryHotState {
    /// A new, empty store (mode defaults to dry-run).
    #[must_use]
    pub fn new() -> InMemoryHotState {
        InMemoryHotState::default()
    }

    /// Number of symbols currently cached.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.inner.lock().expect("hot state mutex").tops.len()
    }
}

impl HotState for InMemoryHotState {
    fn update_top(&self, update: BookUpdate) {
        self.inner
            .lock()
            .expect("hot state mutex")
            .tops
            .insert(update.symbol, update);
    }

    fn top(&self, symbol: &Symbol) -> Option<BookUpdate> {
        self.inner
            .lock()
            .expect("hot state mutex")
            .tops
            .get(symbol)
            .copied()
    }

    fn set_mode(&self, mode: EngineMode) {
        self.inner.lock().expect("hot state mutex").mode = mode;
    }

    fn mode(&self) -> EngineMode {
        self.inner.lock().expect("hot state mutex").mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{BookStatus, Decimal, Nanos, PriceLevel, SeqNo};

    fn update(bid: &str) -> BookUpdate {
        BookUpdate {
            symbol: Symbol::from_parts("BTC", "USDT").unwrap(),
            seq: SeqNo(1),
            update_id: 1,
            status: BookStatus::Live,
            best_bid: Some(PriceLevel::new(
                Decimal::from_str_exact(bid).unwrap(),
                Decimal::from_int(1),
            )),
            best_ask: None,
            ts: Nanos(0),
        }
    }

    #[test]
    fn stores_and_overwrites_top() {
        let hs = InMemoryHotState::new();
        let sym = Symbol::from_parts("BTC", "USDT").unwrap();
        assert!(hs.top(&sym).is_none());
        hs.update_top(update("100"));
        assert_eq!(
            hs.top(&sym).unwrap().best_bid.unwrap().price,
            Decimal::from_int(100)
        );
        hs.update_top(update("101"));
        assert_eq!(
            hs.top(&sym).unwrap().best_bid.unwrap().price,
            Decimal::from_int(101)
        );
        assert_eq!(hs.tracked(), 1);
    }

    #[test]
    fn mode_defaults_dry_run_and_updates() {
        let hs = InMemoryHotState::new();
        assert_eq!(hs.mode(), EngineMode::DryRun);
        hs.set_mode(EngineMode::Live);
        assert_eq!(hs.mode(), EngineMode::Live);
    }
}
