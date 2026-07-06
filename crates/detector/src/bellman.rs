//! Algorithm A: Bellman-Ford negative-cycle detection.
//!
//! A cycle is profitable when the product of its fee-adjusted edge weights
//! exceeds one. Taking negative logarithms turns "product > 1" into "sum of
//! transformed weights < 0", so a profitable cycle of any length is exactly a
//! negative cycle under `cost = -ln(weight)`. Bellman-Ford finds one in
//! `O(V * E)`.
//!
//! This runs on cold start and as a periodic full-graph sanity check against the
//! triangular fast path: if Bellman-Ford ever reports a negative cycle that the
//! fast path missed (for example a length-4 opportunity, when longer cycles are
//! enabled), that is a signal worth surfacing. It is not the hot path.
//!
//! Distances are initialized to zero for every vertex, which is equivalent to
//! adding a zero-cost virtual source to all vertices and detects a negative
//! cycle anywhere in the graph. A small epsilon guards against reporting cycles
//! that are only "negative" through floating-point noise.

use graph::{AssetGraph, VertexId};

/// Default epsilon: only cycles whose log-gain beats this are reported, which
/// keeps floating-point noise from masquerading as arbitrage. The precise,
/// exact-arithmetic gate lives downstream in the evaluator.
pub const DEFAULT_EPSILON: f64 = 1e-9;

/// A detected cycle: vertices and edges in cycle order, with the realized
/// product of fee-adjusted weights.
#[derive(Debug, Clone, PartialEq)]
pub struct Cycle {
    /// Vertices in cycle order.
    pub vertices: Vec<VertexId>,
    /// Edge ids in cycle order (edge `i` goes from `vertices[i]` to
    /// `vertices[(i + 1) % len]`).
    pub edges: Vec<usize>,
    /// Product of the fee-adjusted edge weights (the gross return factor, > 1
    /// for a real opportunity).
    pub product: f64,
}

impl Cycle {
    /// Number of legs in the cycle.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the cycle is empty (never true for a returned cycle).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}

/// Finds a profitable (negative-cost) cycle in the graph, if one exists, using
/// the default epsilon.
#[must_use]
pub fn find_negative_cycle(graph: &AssetGraph) -> Option<Cycle> {
    find_negative_cycle_eps(graph, DEFAULT_EPSILON)
}

/// Finds a profitable cycle with an explicit epsilon.
#[must_use]
pub fn find_negative_cycle_eps(graph: &AssetGraph, epsilon: f64) -> Option<Cycle> {
    let n = graph.num_assets();
    if n == 0 {
        return None;
    }

    // Cost edges: (from, to, -ln(weight), edge_id). Only usable edges (positive,
    // finite weight) participate; a zeroed edge (stale/empty side) is dropped.
    let cost_edges: Vec<(usize, usize, f64, usize)> = graph
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(id, e)| {
            let w = e.weight();
            if w > 0.0 && w.is_finite() {
                Some((e.from() as usize, e.to() as usize, -w.ln(), id))
            } else {
                None
            }
        })
        .collect();

    let mut dist = vec![0.0_f64; n];
    let mut pred_vertex = vec![usize::MAX; n];
    let mut updated = usize::MAX;

    for _ in 0..n {
        updated = usize::MAX;
        for &(u, v, cost, _edge_id) in &cost_edges {
            if dist[u] + cost < dist[v] - epsilon {
                dist[v] = dist[u] + cost;
                pred_vertex[v] = u;
                updated = v;
            }
        }
        if updated == usize::MAX {
            // Converged with no relaxation: no negative cycle.
            return None;
        }
    }

    // `updated` was relaxed on the nth pass, so it is on or reachable from a
    // negative cycle. Stepping back n times lands strictly inside the cycle.
    let mut cursor = updated;
    for _ in 0..n {
        cursor = pred_vertex[cursor];
        if cursor == usize::MAX {
            return None; // defensive: malformed predecessor chain
        }
    }

    reconstruct(graph, cursor, &pred_vertex)
}

/// Reconstructs the cycle from a vertex known to be inside it.
///
/// The predecessor chain runs backward along edges: `pred[c]` is the vertex with
/// an edge into `c`. We walk it to recover the loop, then order the vertices so
/// that consecutive pairs are joined by a real forward edge, which we look up by
/// endpoints.
fn reconstruct(graph: &AssetGraph, start: usize, pred_vertex: &[usize]) -> Option<Cycle> {
    let n = graph.num_assets();

    // Backward chain: chain[0] = start, chain[i+1] = pred(chain[i]).
    let mut chain = vec![start];
    let mut cursor = pred_vertex[start];
    while cursor != start {
        if cursor == usize::MAX || chain.len() > n {
            return None;
        }
        chain.push(cursor);
        cursor = pred_vertex[cursor];
    }

    // Forward order: start, then the rest reversed, so each hop follows a real
    // edge (pred(a) = b means an edge b -> a).
    let m = chain.len();
    let mut vertices: Vec<VertexId> = Vec::with_capacity(m);
    vertices.push(chain[0] as VertexId);
    for &v in chain[1..].iter().rev() {
        vertices.push(v as VertexId);
    }

    let mut edges = Vec::with_capacity(m);
    for i in 0..m {
        let from = vertices[i];
        let to = vertices[(i + 1) % m];
        edges.push(graph.edge_between(from, to)?);
    }

    let product = edges.iter().map(|&e| graph.edge_weight(e)).product::<f64>();

    Some(Cycle {
        vertices,
        edges,
        product,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{BookStatus, BookUpdate, Decimal, Nanos, PriceLevel, SeqNo, Symbol};

    fn sym(base: &str, quote: &str) -> Symbol {
        Symbol::from_parts(base, quote).unwrap()
    }

    fn book(symbol: Symbol, bid: &str, ask: &str) -> BookUpdate {
        BookUpdate {
            symbol,
            seq: SeqNo(0),
            update_id: 1,
            status: BookStatus::Live,
            best_bid: Some(PriceLevel::new(
                Decimal::from_str_exact(bid).unwrap(),
                Decimal::from_int(10),
            )),
            best_ask: Some(PriceLevel::new(
                Decimal::from_str_exact(ask).unwrap(),
                Decimal::from_int(10),
            )),
            ts: Nanos(0),
        }
    }

    fn graph_with(eth_btc: &str, fee: f64) -> AssetGraph {
        let specs = vec![
            (sym("BTC", "USDT"), fee),
            (sym("ETH", "USDT"), fee),
            (sym("ETH", "BTC"), fee),
        ];
        let mut g = AssetGraph::from_specs(&specs);
        g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60000"));
        g.update_from_book(&book(sym("ETH", "USDT"), "3000", "3000"));
        g.update_from_book(&book(sym("ETH", "BTC"), eth_btc, eth_btc));
        g
    }

    #[test]
    fn finds_negative_cycle_for_arbitrage() {
        let g = graph_with("0.052", 0.0); // rich cross => arbitrage
        let cycle = find_negative_cycle(&g).expect("should find a negative cycle");
        assert_eq!(cycle.len(), 3, "triangular arbitrage is a 3-cycle");
        assert!(cycle.product > 1.0, "product = {}", cycle.product);
        // The reconstructed edges form a closed loop.
        for i in 0..cycle.len() {
            let edge = g.edge(cycle.edges[i]).unwrap();
            let next = cycle.vertices[(i + 1) % cycle.len()];
            assert_eq!(edge.from(), cycle.vertices[i]);
            assert_eq!(edge.to(), next);
        }
    }

    #[test]
    fn no_cycle_for_fair_prices_with_fees() {
        let g = graph_with("0.05", 0.001); // fair minus fees => no arbitrage
        assert!(find_negative_cycle(&g).is_none());
    }

    #[test]
    fn empty_graph_has_no_cycle() {
        let g = AssetGraph::from_specs(&[]);
        assert!(find_negative_cycle(&g).is_none());
    }
}
