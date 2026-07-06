//! Arbitrage cycle detection.
//!
//! A cycle is profitable when the product of its fee-adjusted edge weights
//! exceeds one, equivalently a negative cycle under `cost = -ln(weight)`. Three
//! strategies, in the order the specification prescribes:
//!
//!   * [`triangle`]: Algorithm C, the triangular fast path and the default.
//!     [`triangle::scan_all`] for cold start; [`triangle::scan_affected`] for the
//!     incremental hot path (Algorithm B over the precomputed triangle set).
//!   * [`bellman`]: Algorithm A, full-graph Bellman-Ford negative-cycle
//!     detection with witness reconstruction, for cold start and periodic
//!     sanity checks and for cycles longer than three when enabled.
//!
//! Detection produces candidate cycles only. Fees are already baked into the
//! edge weights by the graph; slippage, expected value, confidence filtering,
//! and sizing are the evaluator's job (M6).
#![forbid(unsafe_code)]

pub mod bellman;
pub mod triangle;

pub use bellman::{find_negative_cycle, find_negative_cycle_eps, Cycle, DEFAULT_EPSILON};
pub use triangle::{
    scan_affected, scan_all, triangle_legs, triangle_path, Leg, TriangleHit, DEFAULT_MIN_PRODUCT,
};
