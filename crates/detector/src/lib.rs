//! Arbitrage cycle detection.
//!
//! Built at M5. A profitable cycle is one whose product of fee-adjusted edge
//! weights exceeds 1, equivalently a negative cycle under the `-log(w)`
//! transform. Three algorithms:
//!   * Algorithm C, triangle fast path (default): precomputed length-3 cycles,
//!     three multiplications and a compare per candidate. Built first.
//!   * Algorithm A, Bellman-Ford (O(V*E)): full-graph negative-cycle detection
//!     for cold start and periodic sanity checks, with witness reconstruction.
//!   * Algorithm B, incremental re-evaluation: on a single edge update, only
//!     re-check the bounded set of cycles containing that edge.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
