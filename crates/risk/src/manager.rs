//! The risk manager: pre-trade checks, exposure tracking, circuit breakers, and
//! the shared kill switch.
//!
//! Hard limits are checked before every execution and can veto it. The circuit
//! breaker trips after too many failed executions and forces safe (dry-run)
//! mode. The kill switch is a single shared boolean settable from three
//! independent channels (UI, CLI, HTTP): all three clone [`RiskManager::kill_switch`]
//! and flip it, and every check honors it.
//!
//! The manager runs on the single-threaded engine hot path (so exposure updates
//! take `&mut self`), while the kill switch is an atomic that any thread can
//! set.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use core_types::{Asset, Decimal};

use crate::limits::{BreakerState, RiskBreach, RiskConfig, RiskDecision, SoftWarning};

const TRADE_WINDOW_MS: u64 = 60_000;

// Soft-limit thresholds.
const GHOST_RATE_WARN: f64 = 0.85;
const REJECT_RATE_WARN: f64 = 0.05;
const SLIPPAGE_RATIO_WARN: f64 = 1.5;
const SEQ_GAP_RATE_WARN: f64 = 0.02;

/// Enforces risk limits and owns the kill switch.
pub struct RiskManager {
    config: RiskConfig,
    kill: Arc<AtomicBool>,
    safe_mode: bool,
    open_notional: Decimal,
    per_asset: HashMap<Asset, Decimal>,
    trade_times: VecDeque<u64>,
    realized_pnl: Decimal,
    consecutive_failures: u32,
    failure_times: VecDeque<u64>,
}

impl RiskManager {
    /// Builds a manager with its own kill switch.
    #[must_use]
    pub fn new(config: RiskConfig) -> RiskManager {
        RiskManager::with_kill_switch(config, Arc::new(AtomicBool::new(false)))
    }

    /// Builds a manager sharing an existing kill switch (so the control API,
    /// CLI, and HTTP endpoint all flip the same flag).
    #[must_use]
    pub fn with_kill_switch(config: RiskConfig, kill: Arc<AtomicBool>) -> RiskManager {
        RiskManager {
            config,
            kill,
            safe_mode: false,
            open_notional: Decimal::ZERO,
            per_asset: HashMap::new(),
            trade_times: VecDeque::new(),
            realized_pnl: Decimal::ZERO,
            consecutive_failures: 0,
            failure_times: VecDeque::new(),
        }
    }

    /// A clone of the kill switch for another channel to hold and set.
    #[must_use]
    pub fn kill_switch(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.kill)
    }

    /// Whether trading is halted (kill switch engaged or circuit breaker tripped).
    #[must_use]
    pub fn is_halted(&self) -> bool {
        self.kill.load(Ordering::Relaxed) || self.safe_mode
    }

    /// Engages the kill switch from this channel.
    pub fn engage_kill_switch(&self) {
        self.kill.store(true, Ordering::Relaxed);
    }

    /// Clears the kill switch and resets the circuit breaker (operator recovery).
    pub fn clear(&mut self) {
        self.kill.store(false, Ordering::Relaxed);
        self.safe_mode = false;
        self.consecutive_failures = 0;
        self.failure_times.clear();
    }

    /// Current total open notional.
    #[must_use]
    pub fn open_notional(&self) -> Decimal {
        self.open_notional
    }

    /// Current day's realized PnL.
    #[must_use]
    pub fn realized_pnl(&self) -> Decimal {
        self.realized_pnl
    }

    /// Pre-trade check. Vetoes on the first limit breached.
    pub fn check(
        &mut self,
        notional: Decimal,
        asset: Asset,
        opportunity_age_ms: u64,
        now_ms: u64,
    ) -> RiskDecision {
        if self.is_halted() {
            return RiskDecision::Veto(RiskBreach::EmergencyStop);
        }
        if opportunity_age_ms > self.config.max_opportunity_age_ms {
            return RiskDecision::Veto(RiskBreach::OpportunityStale);
        }
        if notional > self.config.max_notional_per_trade {
            return RiskDecision::Veto(RiskBreach::NotionalPerTrade);
        }
        let projected_total = self
            .open_notional
            .checked_add(notional)
            .unwrap_or(Decimal::MAX);
        if projected_total > self.config.max_total_open_notional {
            return RiskDecision::Veto(RiskBreach::TotalOpenNotional);
        }
        let current_asset = self.per_asset.get(&asset).copied().unwrap_or(Decimal::ZERO);
        let projected_asset = current_asset.checked_add(notional).unwrap_or(Decimal::MAX);
        if projected_asset > self.config.max_notional_per_asset {
            return RiskDecision::Veto(RiskBreach::PerAssetNotional);
        }
        self.prune_trades(now_ms);
        if self.trade_times.len() as u32 >= self.config.max_trades_per_minute {
            return RiskDecision::Veto(RiskBreach::TradeRate);
        }
        if self.realized_pnl <= -self.config.max_daily_loss {
            return RiskDecision::Veto(RiskBreach::DailyDrawdown);
        }
        RiskDecision::Approve
    }

    /// Records that a cycle opened, updating exposure and the trade-rate window.
    pub fn on_trade_open(&mut self, notional: Decimal, asset: Asset, now_ms: u64) {
        self.open_notional = self
            .open_notional
            .checked_add(notional)
            .unwrap_or(Decimal::MAX);
        let entry = self.per_asset.entry(asset).or_insert(Decimal::ZERO);
        *entry = entry.checked_add(notional).unwrap_or(Decimal::MAX);
        self.trade_times.push_back(now_ms);
    }

    /// Records that a cycle closed, releasing exposure and booking realized PnL.
    pub fn on_trade_close(&mut self, notional: Decimal, asset: Asset, realized_delta: Decimal) {
        self.open_notional = self
            .open_notional
            .checked_sub(notional)
            .unwrap_or(Decimal::ZERO)
            .max(Decimal::ZERO);
        if let Some(entry) = self.per_asset.get_mut(&asset) {
            *entry = entry
                .checked_sub(notional)
                .unwrap_or(Decimal::ZERO)
                .max(Decimal::ZERO);
        }
        self.realized_pnl = self
            .realized_pnl
            .checked_add(realized_delta)
            .unwrap_or(self.realized_pnl);
    }

    /// Records an execution result and updates the circuit breaker.
    pub fn record_result(&mut self, success: bool, now_ms: u64) -> BreakerState {
        if success {
            self.consecutive_failures = 0;
            return BreakerState::Ok;
        }
        self.consecutive_failures += 1;
        self.failure_times.push_back(now_ms);
        self.prune_failures(now_ms);
        if self.consecutive_failures >= self.config.max_consecutive_failures
            || self.failure_times.len() as u32 >= self.config.max_failures_in_window
        {
            self.safe_mode = true;
            tracing::error!(
                consecutive = self.consecutive_failures,
                in_window = self.failure_times.len(),
                "circuit breaker tripped; forcing safe (dry-run) mode"
            );
            return BreakerState::Tripped;
        }
        BreakerState::Ok
    }

    /// Resets the daily counters (called at the day boundary).
    pub fn reset_day(&mut self) {
        self.realized_pnl = Decimal::ZERO;
    }

    /// Evaluates soft limits, returning any warnings to surface. These never
    /// veto a trade.
    #[must_use]
    pub fn check_soft(
        &self,
        ghost_rate: f64,
        reject_rate: f64,
        slippage_ratio: f64,
        seq_gap_rate: f64,
    ) -> Vec<SoftWarning> {
        let mut warnings = Vec::new();
        if ghost_rate > GHOST_RATE_WARN {
            warnings.push(SoftWarning::ElevatedGhostRate);
        }
        if reject_rate > REJECT_RATE_WARN {
            warnings.push(SoftWarning::ElevatedRejectRate);
        }
        if slippage_ratio > SLIPPAGE_RATIO_WARN {
            warnings.push(SoftWarning::ElevatedSlippage);
        }
        if seq_gap_rate > SEQ_GAP_RATE_WARN {
            warnings.push(SoftWarning::SequenceGaps);
        }
        warnings
    }

    fn prune_trades(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(TRADE_WINDOW_MS);
        while self.trade_times.front().is_some_and(|&t| t < cutoff) {
            self.trade_times.pop_front();
        }
    }

    fn prune_failures(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.config.failure_window_ms);
        while self.failure_times.front().is_some_and(|&t| t < cutoff) {
            self.failure_times.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: i64) -> Decimal {
        Decimal::from_int(n)
    }

    fn asset(s: &str) -> Asset {
        Asset::new(s).unwrap()
    }

    fn mgr() -> RiskManager {
        RiskManager::new(RiskConfig::default())
    }

    #[test]
    fn approves_within_limits() {
        let mut m = mgr();
        assert!(m.check(d(100), asset("USDT"), 10, 1000).is_approved());
    }

    #[test]
    fn vetoes_per_trade_notional() {
        let mut m = mgr();
        assert_eq!(
            m.check(d(600), asset("USDT"), 10, 1000).breach(),
            Some(RiskBreach::NotionalPerTrade)
        );
    }

    #[test]
    fn vetoes_stale_opportunity() {
        let mut m = mgr();
        assert_eq!(
            m.check(d(100), asset("USDT"), 5000, 1000).breach(),
            Some(RiskBreach::OpportunityStale)
        );
    }

    #[test]
    fn vetoes_per_asset_and_total_notional() {
        let mut m = mgr();
        // Open 1000 in BTC (== per-asset limit). A further BTC trade breaches the
        // per-asset limit first.
        m.on_trade_open(d(1000), asset("BTC"), 1000);
        assert_eq!(
            m.check(d(100), asset("BTC"), 10, 1000).breach(),
            Some(RiskBreach::PerAssetNotional)
        );
        // Add 900 in ETH (total 1900). A 200 trade in a third asset is within the
        // per-trade and per-asset limits but breaches the 2000 total-open limit.
        m.on_trade_open(d(900), asset("ETH"), 1000);
        assert_eq!(
            m.check(d(200), asset("USDT"), 10, 1000).breach(),
            Some(RiskBreach::TotalOpenNotional)
        );
    }

    #[test]
    fn vetoes_trade_rate() {
        let cfg = RiskConfig {
            max_trades_per_minute: 3,
            ..Default::default()
        };
        let mut m = RiskManager::with_kill_switch(cfg, Arc::new(AtomicBool::new(false)));
        for i in 0..3 {
            m.on_trade_open(d(10), asset("USDT"), 1000 + i);
        }
        assert_eq!(
            m.check(d(10), asset("USDT"), 10, 1000).breach(),
            Some(RiskBreach::TradeRate)
        );
        // After the window passes, the rate resets.
        assert!(m
            .check(d(10), asset("USDT"), 10, 1000 + TRADE_WINDOW_MS + 1)
            .is_approved());
    }

    #[test]
    fn vetoes_daily_drawdown() {
        let mut m = mgr();
        m.on_trade_close(d(0), asset("USDT"), d(-500));
        assert_eq!(
            m.check(d(10), asset("USDT"), 10, 1000).breach(),
            Some(RiskBreach::DailyDrawdown)
        );
    }

    #[test]
    fn kill_switch_from_any_channel_vetoes() {
        let mut m = mgr();
        let channel = m.kill_switch(); // e.g. the HTTP /panic endpoint
        channel.store(true, Ordering::Relaxed);
        assert_eq!(
            m.check(d(10), asset("USDT"), 10, 1000).breach(),
            Some(RiskBreach::EmergencyStop)
        );
    }

    #[test]
    fn circuit_breaker_trips_on_consecutive_failures() {
        let mut m = mgr(); // max_consecutive_failures = 3
        assert_eq!(m.record_result(false, 1), BreakerState::Ok);
        assert_eq!(m.record_result(false, 2), BreakerState::Ok);
        assert_eq!(m.record_result(false, 3), BreakerState::Tripped);
        assert!(m.is_halted());
        // A success does not un-trip safe mode until cleared.
        m.record_result(true, 4);
        assert!(m.is_halted());
        m.clear();
        assert!(!m.is_halted());
    }

    #[test]
    fn success_resets_consecutive_failures() {
        let mut m = mgr();
        m.record_result(false, 1);
        m.record_result(false, 2);
        m.record_result(true, 3); // reset
        assert_eq!(m.record_result(false, 4), BreakerState::Ok);
        assert!(!m.is_halted());
    }

    #[test]
    fn exposure_releases_on_close() {
        let mut m = mgr();
        m.on_trade_open(d(300), asset("BTC"), 1000);
        assert_eq!(m.open_notional(), d(300));
        m.on_trade_close(d(300), asset("BTC"), d(5));
        assert_eq!(m.open_notional(), d(0));
        assert_eq!(m.realized_pnl(), d(5));
    }

    #[test]
    fn soft_warnings() {
        let m = mgr();
        let w = m.check_soft(0.9, 0.06, 1.6, 0.03);
        assert!(w.contains(&SoftWarning::ElevatedGhostRate));
        assert!(w.contains(&SoftWarning::ElevatedRejectRate));
        assert!(w.contains(&SoftWarning::ElevatedSlippage));
        assert!(w.contains(&SoftWarning::SequenceGaps));
        assert!(m.check_soft(0.5, 0.0, 1.0, 0.0).is_empty());
    }
}
