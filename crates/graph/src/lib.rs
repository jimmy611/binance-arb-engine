//! Currency graph: assets as vertices, fee-adjusted market pairs as directed
//! weighted edges, with precomputed triangles and a per-symbol affected-triangle
//! index for incremental re-evaluation.
//!
//! See [`AssetGraph`] for the model and the numeric rationale (weights are
//! `f64` rates; amounts elsewhere stay exact fixed-point). The detector crate
//! consumes this to find profitable cycles.
#![forbid(unsafe_code)]

pub mod graph;

pub use graph::{AssetGraph, Edge, EdgeSide, Triangle, VertexId};
