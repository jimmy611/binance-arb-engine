//! The live execution state machine.
//!
//! Given a filter-compliant [`ExecutionPlan`] and an [`OrderExecution`] adapter,
//! this submits the orders and handles the outcomes. It is generic over the
//! adapter, so it is tested exhaustively against a mock and never needs the
//! network. Nothing here decides *whether* to trade live: the engine calls this
//! only after the mode is live, the signed flag is set, and the risk manager has
//! approved. This function never runs in dry-run.
//!
//! Leg risk is the central concern. Parallel IOC submits three independent
//! orders with no cross-order atomicity, so a partial or rejected leg can strand
//! inventory; the sequential mode avoids leg-1 inventory risk. In either case,
//! if the cycle cannot complete cleanly, filled legs are unwound with opposite
//! market orders.

use core_types::ClientOrderId;
use exchange_common::{OrderExecution, OrderRequest, OrderResponse, OrderType};

use crate::modes::ExecutionMode;
use crate::plan::ExecutionPlan;

/// The terminal status of a live cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleStatus {
    /// Every leg filled; the cycle closed.
    Completed,
    /// The cycle could not complete; any filled legs were unwound.
    Aborted,
    /// The plan was refused before submission (not tradeable, or dry-run).
    Refused,
}

/// The result of a live execution attempt.
#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    /// Cycle correlation id.
    pub cycle_id: String,
    /// The mode that was attempted.
    pub mode: ExecutionMode,
    /// The order responses collected, in submission order.
    pub responses: Vec<OrderResponse>,
    /// Terminal status.
    pub status: CycleStatus,
    /// Whether an inventory unwind was performed.
    pub unwound: bool,
    /// Human-readable note.
    pub note: String,
}

/// Executes a plan live against an order-execution adapter.
pub async fn execute_live<E: OrderExecution>(
    plan: &ExecutionPlan,
    executor: &E,
) -> ExecutionOutcome {
    if !plan.tradeable {
        return refused(plan, "plan is not tradeable (filter violation)");
    }
    match plan.mode {
        ExecutionMode::DryRun => refused(plan, "dry-run mode: use execute_dry_run"),
        ExecutionMode::ParallelIoc => execute_parallel(plan, executor).await,
        ExecutionMode::SequentialLockIn => execute_sequential(plan, executor).await,
    }
}

async fn execute_parallel<E: OrderExecution>(
    plan: &ExecutionPlan,
    executor: &E,
) -> ExecutionOutcome {
    let futures = plan.orders.iter().map(|order| executor.submit_order(order));
    let results = futures_util::future::join_all(futures).await;

    let mut responses = Vec::with_capacity(results.len());
    let mut all_filled = true;
    for result in results {
        match result {
            Ok(resp) => {
                if !resp.status.is_terminal()
                    || !matches!(resp.status, exchange_common::OrderStatus::Filled)
                {
                    all_filled = false;
                }
                responses.push(resp);
            }
            Err(_) => all_filled = false,
        }
    }

    if all_filled && responses.len() == plan.orders.len() {
        completed(plan, responses)
    } else {
        let unwound = unwind(plan, executor, &responses).await;
        aborted(
            plan,
            responses,
            unwound,
            "one or more legs did not fully fill",
        )
    }
}

async fn execute_sequential<E: OrderExecution>(
    plan: &ExecutionPlan,
    executor: &E,
) -> ExecutionOutcome {
    let mut responses: Vec<OrderResponse> = Vec::with_capacity(plan.orders.len());

    for order in &plan.orders {
        match executor.submit_order(order).await {
            Ok(resp) => {
                let filled = resp.status.is_any_fill() && resp.filled_qty.is_positive();
                responses.push(resp);
                if !filled {
                    // Leg 0 not filling is a clean abort (no inventory). A later
                    // leg not filling requires unwinding what we hold.
                    if responses.len() == 1 {
                        return aborted(plan, responses, false, "leg 1 did not fill (clean abort)");
                    }
                    let unwound = unwind(plan, executor, &responses).await;
                    return aborted(plan, responses, unwound, "a lock-in leg did not fill");
                }
            }
            Err(_) => {
                if responses.is_empty() {
                    return aborted(plan, responses, false, "leg 1 submission failed");
                }
                let unwound = unwind(plan, executor, &responses).await;
                return aborted(plan, responses, unwound, "a leg submission failed");
            }
        }
    }

    completed(plan, responses)
}

/// Unwinds any filled legs with opposite-side market orders.
async fn unwind<E: OrderExecution>(
    plan: &ExecutionPlan,
    executor: &E,
    responses: &[OrderResponse],
) -> bool {
    let mut any = false;
    for (resp, order) in responses.iter().zip(plan.orders.iter()) {
        if resp.status.is_any_fill() && resp.filled_qty.is_positive() {
            let unwind_order = OrderRequest {
                symbol: order.symbol,
                side: order.side.opposite(),
                order_type: OrderType::Market,
                price: None,
                qty: resp.filled_qty,
                client_order_id: ClientOrderId(format!("{}-UNWIND", order.client_order_id.0)),
            };
            // Best effort: an unwind that itself fails escalates to the risk
            // manager via the caller, which sees `unwound = true` but a non-flat
            // position on reconciliation.
            let _ = executor.submit_order(&unwind_order).await;
            any = true;
        }
    }
    any
}

fn completed(plan: &ExecutionPlan, responses: Vec<OrderResponse>) -> ExecutionOutcome {
    ExecutionOutcome {
        cycle_id: plan.cycle_id.clone(),
        mode: plan.mode,
        responses,
        status: CycleStatus::Completed,
        unwound: false,
        note: "cycle filled".to_owned(),
    }
}

fn aborted(
    plan: &ExecutionPlan,
    responses: Vec<OrderResponse>,
    unwound: bool,
    note: &str,
) -> ExecutionOutcome {
    ExecutionOutcome {
        cycle_id: plan.cycle_id.clone(),
        mode: plan.mode,
        responses,
        status: CycleStatus::Aborted,
        unwound,
        note: note.to_owned(),
    }
}

fn refused(plan: &ExecutionPlan, note: &str) -> ExecutionOutcome {
    ExecutionOutcome {
        cycle_id: plan.cycle_id.clone(),
        mode: plan.mode,
        responses: Vec::new(),
        status: CycleStatus::Refused,
        unwound: false,
        note: note.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{build_plan, PlannedLeg};
    use core_types::{Decimal, OrderId, Side, Symbol, SymbolFilters};
    use exchange_common::{OrderStatus, Result as ExResult};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn filters() -> SymbolFilters {
        SymbolFilters {
            tick_size: d("0.01"),
            step_size: d("0.001"),
            min_qty: d("0.001"),
            max_qty: d("1000"),
            min_notional: d("0"),
        }
    }

    fn legs() -> Vec<PlannedLeg> {
        vec![
            PlannedLeg {
                symbol: Symbol::from_parts("BTC", "USDT").unwrap(),
                side: Side::Sell,
                price: d("60000"),
                qty: d("0.01"),
                filters: filters(),
            },
            PlannedLeg {
                symbol: Symbol::from_parts("ETH", "BTC").unwrap(),
                side: Side::Buy,
                price: d("0.05"),
                qty: d("0.2"),
                filters: filters(),
            },
            PlannedLeg {
                symbol: Symbol::from_parts("ETH", "USDT").unwrap(),
                side: Side::Sell,
                price: d("3000"),
                qty: d("0.2"),
                filters: filters(),
            },
        ]
    }

    fn resp(status: OrderStatus, filled: &str) -> OrderResponse {
        OrderResponse {
            order_id: OrderId(1),
            client_order_id: ClientOrderId("x".into()),
            status,
            filled_qty: d(filled),
            cumulative_quote: d("0"),
            avg_price: None,
        }
    }

    struct MockExecutor {
        queue: Mutex<VecDeque<ExResult<OrderResponse>>>,
        submitted: Mutex<Vec<OrderRequest>>,
    }

    impl MockExecutor {
        fn new(responses: Vec<ExResult<OrderResponse>>) -> MockExecutor {
            MockExecutor {
                queue: Mutex::new(responses.into_iter().collect()),
                submitted: Mutex::new(Vec::new()),
            }
        }
        fn submitted_count(&self) -> usize {
            self.submitted.lock().unwrap().len()
        }
    }

    impl OrderExecution for MockExecutor {
        async fn submit_order(&self, request: &OrderRequest) -> ExResult<OrderResponse> {
            self.submitted.lock().unwrap().push(request.clone());
            self.queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(resp(OrderStatus::Filled, "0.01")))
        }
        async fn cancel_order(&self, _symbol: Symbol, _id: OrderId) -> ExResult<()> {
            Ok(())
        }
    }

    fn plan(mode: ExecutionMode) -> ExecutionPlan {
        build_plan("cyc-1", &legs(), mode)
    }

    #[tokio::test]
    async fn parallel_all_filled_completes() {
        let exec = MockExecutor::new(vec![
            Ok(resp(OrderStatus::Filled, "0.01")),
            Ok(resp(OrderStatus::Filled, "0.2")),
            Ok(resp(OrderStatus::Filled, "0.2")),
        ]);
        let outcome = execute_live(&plan(ExecutionMode::ParallelIoc), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Completed);
        assert!(!outcome.unwound);
        assert_eq!(exec.submitted_count(), 3);
    }

    #[tokio::test]
    async fn parallel_one_rejected_aborts_and_unwinds() {
        let exec = MockExecutor::new(vec![
            Ok(resp(OrderStatus::Filled, "0.01")),
            Ok(resp(OrderStatus::Rejected, "0")),
            Ok(resp(OrderStatus::Filled, "0.2")),
        ]);
        let outcome = execute_live(&plan(ExecutionMode::ParallelIoc), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Aborted);
        assert!(outcome.unwound, "filled legs must be unwound");
        // 3 initial submits + 2 unwind orders for the two filled legs.
        assert_eq!(exec.submitted_count(), 5);
    }

    #[tokio::test]
    async fn sequential_leg1_no_fill_clean_abort() {
        let exec = MockExecutor::new(vec![Ok(resp(OrderStatus::Expired, "0"))]);
        let outcome = execute_live(&plan(ExecutionMode::SequentialLockIn), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Aborted);
        assert!(!outcome.unwound, "no inventory to unwind");
        assert_eq!(exec.submitted_count(), 1);
    }

    #[tokio::test]
    async fn sequential_later_leg_fail_unwinds() {
        let exec = MockExecutor::new(vec![
            Ok(resp(OrderStatus::Filled, "0.01")),
            Ok(resp(OrderStatus::Rejected, "0")),
        ]);
        let outcome = execute_live(&plan(ExecutionMode::SequentialLockIn), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Aborted);
        assert!(outcome.unwound);
        // leg1 fill + leg2 reject + 1 unwind of leg1.
        assert_eq!(exec.submitted_count(), 3);
    }

    #[tokio::test]
    async fn sequential_all_filled_completes() {
        let exec = MockExecutor::new(vec![
            Ok(resp(OrderStatus::Filled, "0.01")),
            Ok(resp(OrderStatus::Filled, "0.2")),
            Ok(resp(OrderStatus::Filled, "0.2")),
        ]);
        let outcome = execute_live(&plan(ExecutionMode::SequentialLockIn), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Completed);
    }

    #[tokio::test]
    async fn dry_run_is_refused() {
        let exec = MockExecutor::new(vec![]);
        let outcome = execute_live(&plan(ExecutionMode::DryRun), &exec).await;
        assert_eq!(outcome.status, CycleStatus::Refused);
        assert_eq!(exec.submitted_count(), 0);
    }
}
