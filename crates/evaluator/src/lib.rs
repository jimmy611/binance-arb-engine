//! Opportunity evaluation: turn a detected cycle into an actionable decision.
//!
//! Built at M6. Applies the tier-based fee model, computes volume-weighted
//! effective rates by walking order-book levels, finds the profit-maximising
//! starting notional by evaluating breakpoints (profit is concave
//! piecewise-linear in size, so the optimum is exact and O(depth)), estimates
//! latency-adjusted expected value, applies the statistical confidence filter
//! (persistence, depth confidence, survival probability), and sizes with a
//! fractional Kelly criterion capped by hard risk limits.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
