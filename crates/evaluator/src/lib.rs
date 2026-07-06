//! Opportunity evaluation: turn a detected cycle into an actionable, sized
//! decision.
//!
//! Detection (M5) says a cycle is gross-profitable at top of book. The evaluator
//! decides whether it is worth trading and how large:
//!
//!   * [`vwap`]: the slippage-aware VWAP optimizer. Composes the three legs'
//!     order-book depth into a concave profit shape and finds the exact
//!     profit-maximizing size at an order-book breakpoint.
//!   * [`fees`]: the tier-based maker/taker fee schedule with per-symbol
//!     overrides.
//!   * [`confidence`]: the persistence, depth-confidence, and latency-adjusted
//!     expected-value filter that rejects ghosts.
//!   * [`kelly`]: fractional Kelly position sizing, capped by hard limits and by
//!     the order-book capacity.
//!
//! Every function is pure and unit-tested; fee and VWAP math carry the exact
//! arithmetic (amounts) and controlled floating point (rates) split from the
//! numeric policy.
#![forbid(unsafe_code)]

pub mod confidence;
pub mod fees;
pub mod kelly;
pub mod vwap;

pub use confidence::{
    evaluate as evaluate_confidence, latency_adjusted_ev, passes_filters, survival_probability,
    ConfidenceParams, ConfidenceSignal,
};
pub use fees::{Fee, FeeSchedule};
pub use kelly::{fractional_kelly_size, size_with_capacity, KellyParams};
pub use vwap::{optimize_cycle, CycleShape, LegBook, Segment};
