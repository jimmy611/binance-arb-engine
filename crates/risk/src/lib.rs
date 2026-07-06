//! Risk management: hard limits, circuit breakers, soft warnings, and the kill
//! switch.
//!
//! The [`RiskManager`] enforces hard limits before every execution (per-trade,
//! total, and per-asset notional; trade rate; daily drawdown; opportunity age)
//! and can veto a trade or force safe (dry-run) mode via its circuit breaker. It
//! owns a single kill switch that three independent channels (UI, CLI, HTTP)
//! share and can flip. Soft limits (elevated ghost/reject/slippage rates,
//! sequence gaps) raise warnings without vetoing.
#![forbid(unsafe_code)]

pub mod limits;
pub mod manager;

pub use limits::{BreakerState, RiskBreach, RiskConfig, RiskDecision, SoftWarning};
pub use manager::RiskManager;
