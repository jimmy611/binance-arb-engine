//! Currency graph: assets as vertices, market pairs as directed weighted edges.
//!
//! Built at M4. For every pair (BASE, QUOTE) it maintains two directed edges
//! (BASE->QUOTE at the fee-adjusted bid, QUOTE->BASE at the fee-adjusted inverse
//! ask), updated incrementally on every book update. Precomputes the set of all
//! valid triangles at startup and, per edge, the triangles that edge
//! participates in, so a single book update touches only the affected cycles.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
