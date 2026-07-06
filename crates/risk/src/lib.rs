//! Risk manager: hard limits, soft warnings, circuit breakers, and kill switch.
//!
//! Built at M11. Enforces hard limits before every execution (max notional per
//! trade, total open notional, per-asset notional, trades per minute, daily
//! drawdown, opportunity max age, minimum quote balances) and can veto an
//! execution or force the engine into safe (dry-run) mode. Raises soft-limit
//! warnings (elevated slippage, ghost rate, reject rate, sequence gaps) without
//! vetoing. Owns the single `emergency_stop` boolean settable from three
//! independent channels (UI, CLI, HTTP endpoint).
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
