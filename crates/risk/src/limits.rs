//! Risk configuration and decision types.

use core_types::Decimal;
use serde::{Deserialize, Serialize};

/// Hard and circuit-breaker limits. Notional limits are in the base quote
/// currency (USDT by default).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum notional for a single cycle.
    pub max_notional_per_trade: Decimal,
    /// Maximum total open notional across all positions.
    pub max_total_open_notional: Decimal,
    /// Maximum open notional concentrated in a single asset.
    pub max_notional_per_asset: Decimal,
    /// Maximum trades per rolling minute.
    pub max_trades_per_minute: u32,
    /// Maximum cumulative realized loss per day before halting.
    pub max_daily_loss: Decimal,
    /// Maximum age of an opportunity (ms) for it to still be actionable.
    pub max_opportunity_age_ms: u64,
    /// Consecutive failed executions that trip the circuit breaker.
    pub max_consecutive_failures: u32,
    /// Failed executions within the window that trip the circuit breaker.
    pub max_failures_in_window: u32,
    /// The failure-counting window (ms).
    pub failure_window_ms: u64,
}

impl Default for RiskConfig {
    fn default() -> RiskConfig {
        RiskConfig {
            max_notional_per_trade: Decimal::from_int(500),
            max_total_open_notional: Decimal::from_int(2000),
            max_notional_per_asset: Decimal::from_int(1000),
            max_trades_per_minute: 60,
            max_daily_loss: Decimal::from_int(500),
            max_opportunity_age_ms: 500,
            max_consecutive_failures: 3,
            max_failures_in_window: 5,
            failure_window_ms: 60_000,
        }
    }
}

/// The specific limit a check tripped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskBreach {
    /// The emergency stop is engaged (or the circuit breaker forced safe mode).
    EmergencyStop,
    /// The trade notional exceeds the per-trade limit.
    NotionalPerTrade,
    /// Total open notional would exceed the limit.
    TotalOpenNotional,
    /// Per-asset notional would exceed the limit.
    PerAssetNotional,
    /// The trade rate exceeds the per-minute limit.
    TradeRate,
    /// The daily drawdown limit has been reached.
    DailyDrawdown,
    /// The opportunity is older than the maximum actionable age.
    OpportunityStale,
}

/// The result of a pre-trade risk check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskDecision {
    /// The trade may proceed.
    Approve,
    /// The trade is vetoed for the given reason.
    Veto(RiskBreach),
}

impl RiskDecision {
    /// Whether the trade was approved.
    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, RiskDecision::Approve)
    }

    /// The breach, if vetoed.
    #[must_use]
    pub fn breach(self) -> Option<RiskBreach> {
        match self {
            RiskDecision::Veto(b) => Some(b),
            RiskDecision::Approve => None,
        }
    }
}

/// The state of the circuit breaker after recording an execution result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Normal operation.
    Ok,
    /// The breaker tripped on this result; the engine is forced to safe (dry-run)
    /// mode.
    Tripped,
}

/// A non-vetoing soft-limit warning worth surfacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftWarning {
    /// Fill slippage is elevated versus expected.
    ElevatedSlippage,
    /// Too many detected opportunities vanish before they are tradeable.
    ElevatedGhostRate,
    /// Order reject rate is elevated.
    ElevatedRejectRate,
    /// Sequence gaps on a symbol above threshold.
    SequenceGaps,
}
