//! Offline replay harness.
//!
//! Built at M7. Reads WAL files and drives the exact same engine code path in
//! offline mode, reproducing any live session deterministically for debugging,
//! regression testing, and backtesting new algorithm versions against real
//! recorded order flow.
//!
//! Deliberately a no-op at milestone M0.
#![forbid(unsafe_code)]

fn main() {
    eprintln!("arb-replay: replay harness lands at milestone M7 (persistence + replay).");
}
