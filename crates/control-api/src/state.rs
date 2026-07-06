//! Shared control-plane state.
//!
//! Holds the runtime engine mode, the versioned configuration (with optimistic
//! locking), the emergency-stop flag, and bounded ring buffers of recent
//! opportunities, trades, and PnL points that the History service serves. In
//! production the history queries hit TimescaleDB; the in-memory buffers keep
//! the control API useful before that infra is wired and back the current
//! session's view.

use std::collections::VecDeque;
use std::sync::RwLock;

use crate::pb;

/// Maximum entries retained in each in-memory history ring.
const RING_CAP: usize = 2000;

struct Inner {
    running: bool,
    emergency_stopped: bool,
    mode: pb::EngineMode,
    config: pb::EngineConfig,
    opportunities: VecDeque<pb::Opportunity>,
    trades: VecDeque<pb::Trade>,
    pnl: VecDeque<pb::PnlPoint>,
}

/// Thread-safe control-plane state shared by the gRPC services and the engine.
pub struct ControlState {
    inner: RwLock<Inner>,
}

impl Default for ControlState {
    fn default() -> ControlState {
        ControlState::new()
    }
}

impl ControlState {
    /// New state with the default (dry-run) configuration.
    #[must_use]
    pub fn new() -> ControlState {
        ControlState {
            inner: RwLock::new(Inner {
                running: false,
                emergency_stopped: false,
                mode: pb::EngineMode::DryRun,
                config: default_config(),
                opportunities: VecDeque::new(),
                trades: VecDeque::new(),
                pnl: VecDeque::new(),
            }),
        }
    }

    /// The current engine mode.
    #[must_use]
    pub fn mode(&self) -> pb::EngineMode {
        self.inner.read().expect("state lock").mode
    }

    /// Whether the engine is running.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.read().expect("state lock").running
    }

    /// Whether the emergency stop is engaged.
    #[must_use]
    pub fn is_emergency(&self) -> bool {
        self.inner.read().expect("state lock").emergency_stopped
    }

    /// Sets the running flag.
    pub fn set_running(&self, running: bool) {
        self.inner.write().expect("state lock").running = running;
    }

    /// Sets the engine mode. Returns `false` (leaving the mode unchanged) if a
    /// switch to live is attempted while the emergency stop is engaged.
    pub fn set_mode(&self, mode: pb::EngineMode) -> bool {
        let mut inner = self.inner.write().expect("state lock");
        if mode == pb::EngineMode::Live && inner.emergency_stopped {
            return false;
        }
        inner.mode = mode;
        true
    }

    /// Engages the emergency stop, forcing dry-run mode.
    pub fn emergency_stop(&self) {
        let mut inner = self.inner.write().expect("state lock");
        inner.emergency_stopped = true;
        inner.mode = pb::EngineMode::DryRun;
        inner.running = false;
    }

    /// Clears the emergency stop (operator recovery).
    pub fn clear_emergency(&self) {
        self.inner.write().expect("state lock").emergency_stopped = false;
    }

    /// A clone of the current configuration.
    #[must_use]
    pub fn config(&self) -> pb::EngineConfig {
        self.inner.read().expect("state lock").config.clone()
    }

    /// Applies a configuration update with optimistic locking. Returns the new
    /// version on success, or `None` if `expected_version` does not match the
    /// current version (a concurrent update won).
    pub fn update_config(
        &self,
        mut config: pb::EngineConfig,
        expected_version: u64,
    ) -> Option<u64> {
        let mut inner = self.inner.write().expect("state lock");
        if inner.config.version != expected_version {
            return None;
        }
        let new_version = inner.config.version + 1;
        config.version = new_version;
        inner.config = config;
        Some(new_version)
    }

    /// Records an opportunity in the recent-opportunity ring.
    pub fn record_opportunity(&self, opp: pb::Opportunity) {
        let mut inner = self.inner.write().expect("state lock");
        push_bounded(&mut inner.opportunities, opp);
    }

    /// Records a trade in the recent-trade ring.
    pub fn record_trade(&self, trade: pb::Trade) {
        let mut inner = self.inner.write().expect("state lock");
        push_bounded(&mut inner.trades, trade);
    }

    /// Records a PnL point.
    pub fn record_pnl(&self, point: pb::PnlPoint) {
        let mut inner = self.inner.write().expect("state lock");
        push_bounded(&mut inner.pnl, point);
    }

    /// The most recent trades, newest first, up to `limit`.
    #[must_use]
    pub fn recent_trades(&self, limit: usize) -> Vec<pb::Trade> {
        let inner = self.inner.read().expect("state lock");
        inner.trades.iter().rev().take(limit).cloned().collect()
    }

    /// The most recent opportunities with EV at least `min_ev`, newest first.
    #[must_use]
    pub fn recent_opportunities(&self, limit: usize, min_ev: f64) -> Vec<pb::Opportunity> {
        let inner = self.inner.read().expect("state lock");
        inner
            .opportunities
            .iter()
            .rev()
            .filter(|o| o.ev_quote >= min_ev)
            .take(limit)
            .cloned()
            .collect()
    }

    /// The PnL series (oldest first), optionally since a timestamp (nanos).
    #[must_use]
    pub fn pnl_series(&self, since_unix_nanos: u64) -> Vec<pb::PnlPoint> {
        let inner = self.inner.read().expect("state lock");
        inner
            .pnl
            .iter()
            .filter(|p| {
                p.ts.as_ref()
                    .is_none_or(|t| t.unix_nanos >= since_unix_nanos)
            })
            .cloned()
            .collect()
    }
}

fn push_bounded<T>(ring: &mut VecDeque<T>, item: T) {
    if ring.len() >= RING_CAP {
        ring.pop_front();
    }
    ring.push_back(item);
}

fn default_config() -> pb::EngineConfig {
    pb::EngineConfig {
        version: 1,
        default_mode: pb::EngineMode::DryRun as i32,
        max_cycle_length: 3,
        max_notional_per_trade: 500.0,
        max_total_open_notional: 2000.0,
        max_daily_loss: 500.0,
        max_trades_per_minute: 60,
        min_edge_bps: 5.0,
        symbol_allowlist: Vec::new(),
        symbol_denylist: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimistic_locking() {
        let state = ControlState::new();
        assert_eq!(state.config().version, 1);
        // Wrong expected version is rejected.
        assert_eq!(state.update_config(state.config(), 99), None);
        // Correct version succeeds and bumps to 2.
        let new_version = state.update_config(state.config(), 1).unwrap();
        assert_eq!(new_version, 2);
        assert_eq!(state.config().version, 2);
    }

    #[test]
    fn emergency_forces_dry_run_and_blocks_live() {
        let state = ControlState::new();
        assert!(state.set_mode(pb::EngineMode::Live));
        assert_eq!(state.mode(), pb::EngineMode::Live);
        state.emergency_stop();
        assert!(state.is_emergency());
        assert_eq!(state.mode(), pb::EngineMode::DryRun);
        // Cannot go live while stopped.
        assert!(!state.set_mode(pb::EngineMode::Live));
        assert_eq!(state.mode(), pb::EngineMode::DryRun);
    }

    #[test]
    fn history_rings_return_newest_first() {
        let state = ControlState::new();
        for i in 0..3 {
            state.record_trade(pb::Trade {
                cycle_id: format!("c{i}"),
                ..Default::default()
            });
        }
        let trades = state.recent_trades(2);
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].cycle_id, "c2");
        assert_eq!(trades[1].cycle_id, "c1");
    }
}
