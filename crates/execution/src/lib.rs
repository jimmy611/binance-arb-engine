//! Execution router: mode selection, order-plan building, and dry-run routing.
//!
//! Built at M10 (dry-run). Given a sized cycle it chooses an execution mode
//! ([`modes`]), assembles filter-compliant, snapped orders ([`plan`]), and, in
//! dry-run, logs the would-be orders without sending anything ([`router`]).
//! Order signing (Ed25519) lives in the Binance adapter. The live execution
//! state machine ([`live`]) is generic over an [`exchange_common::OrderExecution`]
//! adapter and is fully tested against a mock; the engine only invokes it once
//! the mode is live, the signed flag is set, and the risk manager has approved.
#![forbid(unsafe_code)]

pub mod live;
pub mod modes;
pub mod plan;
pub mod router;

pub use live::{execute_live, CycleStatus, ExecutionOutcome};
pub use modes::{select_mode, ExecutionMode, ModeConfig};
pub use plan::{build_plan, ExecutionPlan, PlannedLeg};
pub use router::{DryRunReport, ExecutionRouter};
