//! Historical simulation / backtest harness.
//!
//! Built at M13. Runs the engine over recorded WAL data with a chosen algorithm
//! configuration and produces a report (equity curve, hit rate, average edge,
//! latency distribution). The operator must run a backtest over at least 24
//! hours of recorded data showing positive expected value at the configured
//! risk parameters before live mode can be enabled.
//!
//! Deliberately a no-op at milestone M0.
#![forbid(unsafe_code)]

fn main() {
    eprintln!("arb-backtest: backtest harness lands at milestone M13.");
}
