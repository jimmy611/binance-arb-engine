//! The engine binary: wires every component together and owns the process
//! lifecycle.
//!
//! At milestone M0 this is a minimal, honest entry point. It does not fake a
//! trading loop. It establishes the two things that must be true from the very
//! first commit: the process starts in the fail-safe [`EngineMode::DryRun`]
//! state, and the wiring point for the real pipeline is clearly marked. Each
//! subsequent milestone attaches a real component here (market data at M2, the
//! book at M3, and so on).
#![forbid(unsafe_code)]

use core_types::EngineMode;

fn main() {
    // The specification's non-negotiable: fail-safe by default. The engine
    // always boots in dry-run; reaching live mode requires a signed config flag
    // and a UI confirmation, wired at M12.
    let mode = EngineMode::default();
    assert_eq!(mode, EngineMode::DryRun, "engine must boot in dry-run");

    let version = env!("CARGO_PKG_VERSION");
    println!("arb-engine v{version} starting in {mode} mode");
    println!(
        "pipeline wiring is attached per milestone; see docs/architecture.md. \
         Nothing is traded and no orders are sent in this build."
    );

    // TODO(M2+): construct the exchange connector, order book, currency graph,
    // detector, evaluator, risk manager, execution router, persistence, and
    // control API, then run the tokio runtime. Not stubbed with fake behaviour
    // on purpose: there is no simulated market loop here.
}
