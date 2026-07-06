//! Execution mode selection.
//!
//! Three modes, per the specification:
//!   * Parallel IOC: all legs submitted at once as IOC limit orders. Fastest
//!     wall-clock, but the legs are independent orders (no cross-order
//!     atomicity), so a partial fill can strand inventory.
//!   * Sequential lock-in: leg 1 as an IOC limit at the detected price, then, on
//!     fill, legs 2 and 3 as market orders (guaranteed fill, worse price). No
//!     leg-1 inventory risk.
//!   * Dry run: nothing is sent; would-be orders are logged. The startup
//!     default.

use serde::{Deserialize, Serialize};

/// How a cycle is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// All legs as parallel IOC limit orders.
    ParallelIoc,
    /// Leg 1 IOC limit, remaining legs market on fill.
    SequentialLockIn,
    /// Log only, never submit.
    DryRun,
}

/// Tunables for choosing a mode per opportunity.
#[derive(Debug, Clone, Copy)]
pub struct ModeConfig {
    /// Whether live trading is enabled. When false, always dry-run.
    pub live: bool,
    /// Below this edge (bps), prefer the lower-risk sequential mode.
    pub prefer_sequential_below_bps: f64,
    /// Minimum depth confidence (profitable size / min notional) to risk the
    /// parallel mode.
    pub min_depth_ratio_for_parallel: f64,
}

impl Default for ModeConfig {
    fn default() -> ModeConfig {
        ModeConfig {
            live: false,
            prefer_sequential_below_bps: 4.0,
            min_depth_ratio_for_parallel: 1.5,
        }
    }
}

/// Chooses an execution mode. Always dry-run when live trading is disabled; a
/// thin edge or shallow depth prefers the lower-risk sequential mode; otherwise
/// the fast parallel mode.
#[must_use]
pub fn select_mode(edge_bps: f64, depth_ratio: f64, config: &ModeConfig) -> ExecutionMode {
    if !config.live {
        return ExecutionMode::DryRun;
    }
    if edge_bps < config.prefer_sequential_below_bps
        || depth_ratio < config.min_depth_ratio_for_parallel
    {
        ExecutionMode::SequentialLockIn
    } else {
        ExecutionMode::ParallelIoc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_live_is_always_dry_run() {
        let cfg = ModeConfig::default(); // live = false
        assert_eq!(select_mode(50.0, 10.0, &cfg), ExecutionMode::DryRun);
    }

    #[test]
    fn thin_edge_prefers_sequential() {
        let cfg = ModeConfig {
            live: true,
            ..Default::default()
        };
        assert_eq!(
            select_mode(2.0, 10.0, &cfg),
            ExecutionMode::SequentialLockIn
        );
    }

    #[test]
    fn shallow_depth_prefers_sequential() {
        let cfg = ModeConfig {
            live: true,
            ..Default::default()
        };
        assert_eq!(
            select_mode(20.0, 1.1, &cfg),
            ExecutionMode::SequentialLockIn
        );
    }

    #[test]
    fn fat_edge_and_depth_go_parallel() {
        let cfg = ModeConfig {
            live: true,
            ..Default::default()
        };
        assert_eq!(select_mode(20.0, 3.0, &cfg), ExecutionMode::ParallelIoc);
    }
}
