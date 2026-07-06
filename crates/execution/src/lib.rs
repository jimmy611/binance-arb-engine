//! Execution router: mode selection, order-plan building, and dry-run routing.
//!
//! Built at M10 (dry-run). Given a sized cycle it chooses an execution mode
//! ([`modes`]), assembles filter-compliant, snapped orders ([`plan`]), and, in
//! dry-run, logs the would-be orders without sending anything ([`router`]).
//! Order signing (Ed25519) lives in the Binance adapter; live submission via
//! [`exchange_common::OrderExecution`] is added at M12 and gated behind the
//! signed live-mode flag. Nothing here sends a live order.
#![forbid(unsafe_code)]

pub mod modes;
pub mod plan;
pub mod router;

pub use modes::{select_mode, ExecutionMode, ModeConfig};
pub use plan::{build_plan, ExecutionPlan, PlannedLeg};
pub use router::{DryRunReport, ExecutionRouter};
