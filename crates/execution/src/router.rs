//! The execution router: choose a mode, build the plan, and (in dry-run) log it.
//!
//! Live submission is deliberately absent here; it arrives at the live-execution
//! milestone as a method that consumes an [`exchange_common::OrderExecution`]
//! adapter. Until then the router only ever plans and logs, which is the
//! fail-safe default the specification requires.

use crate::modes::{select_mode, ModeConfig};
use crate::plan::{build_plan, ExecutionPlan, PlannedLeg};

/// The outcome of a dry-run execution: the plan, and confirmation nothing was
/// sent.
#[derive(Debug, Clone)]
pub struct DryRunReport {
    /// The plan that would have been submitted.
    pub plan: ExecutionPlan,
    /// Always false in dry-run.
    pub submitted: bool,
    /// Why nothing was submitted.
    pub reason: &'static str,
}

/// Routes sized cycles to execution plans.
pub struct ExecutionRouter {
    config: ModeConfig,
}

impl ExecutionRouter {
    /// Builds a router with the given mode configuration.
    #[must_use]
    pub fn new(config: ModeConfig) -> ExecutionRouter {
        ExecutionRouter { config }
    }

    /// The mode configuration.
    #[must_use]
    pub fn config(&self) -> &ModeConfig {
        &self.config
    }

    /// Plans a cycle: chooses a mode from the edge and depth, then assembles the
    /// filter-compliant orders.
    #[must_use]
    pub fn plan_cycle(
        &self,
        cycle_id: &str,
        legs: &[PlannedLeg],
        edge_bps: f64,
        depth_ratio: f64,
    ) -> ExecutionPlan {
        let mode = select_mode(edge_bps, depth_ratio, &self.config);
        build_plan(cycle_id, legs, mode)
    }

    /// Logs every would-be order and returns a report. Sends nothing.
    #[must_use]
    pub fn execute_dry_run(&self, plan: &ExecutionPlan) -> DryRunReport {
        for order in &plan.orders {
            tracing::info!(
                cycle = %plan.cycle_id,
                symbol = %order.symbol,
                side = ?order.side,
                order_type = ?order.order_type,
                qty = %order.qty,
                price = ?order.price.map(|p| p.to_string()),
                "dry-run: would submit order (not sent)"
            );
        }
        DryRunReport {
            plan: plan.clone(),
            submitted: false,
            reason: "dry-run mode",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::ExecutionMode;
    use core_types::{Decimal, Side, Symbol, SymbolFilters};

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn leg() -> PlannedLeg {
        PlannedLeg {
            symbol: Symbol::from_parts("BTC", "USDT").unwrap(),
            side: Side::Sell,
            price: d("60000"),
            qty: d("0.01"),
            filters: SymbolFilters {
                tick_size: d("0.01"),
                step_size: d("0.001"),
                min_qty: d("0.001"),
                max_qty: d("1000"),
                min_notional: d("10"),
            },
        }
    }

    #[test]
    fn not_live_plans_dry_run_and_sends_nothing() {
        let router = ExecutionRouter::new(ModeConfig::default()); // live = false
        let plan = router.plan_cycle("cyc-1", &[leg(), leg(), leg()], 20.0, 3.0);
        assert_eq!(plan.mode, ExecutionMode::DryRun);
        let report = router.execute_dry_run(&plan);
        assert!(!report.submitted);
        assert_eq!(report.plan.orders.len(), 3);
    }

    #[test]
    fn live_config_selects_parallel_for_fat_edge() {
        let router = ExecutionRouter::new(ModeConfig {
            live: true,
            ..Default::default()
        });
        let plan = router.plan_cycle("cyc-2", &[leg(), leg(), leg()], 20.0, 3.0);
        assert_eq!(plan.mode, ExecutionMode::ParallelIoc);
    }
}
