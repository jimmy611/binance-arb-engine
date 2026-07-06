//! The currency graph.
//!
//! Assets are vertices; each trading pair contributes two directed, weighted
//! edges. For a pair `(BASE, QUOTE)`:
//!
//! * `BASE -> QUOTE` weighted by the best bid (selling BASE for QUOTE),
//! * `QUOTE -> BASE` weighted by the inverse of the best ask (buying BASE with
//!   QUOTE),
//!
//! both multiplied by the leg's net-of-fee multiplier `(1 - taker_fee)`. A
//! cycle is profitable when the product of its edge weights exceeds one, so
//! weights are held as `f64` (rates, per the numeric policy: amounts are exact
//! fixed-point, rates are floating point).
//!
//! Edge weights are updated incrementally on every [`BookUpdate`]. The set of
//! length-3 cycles (triangles) is precomputed once, and a per-symbol index maps
//! a symbol to the triangles it participates in, so a single book update only
//! forces re-evaluation of a few triangles rather than the whole graph.

use std::collections::HashMap;

use core_types::{Asset, BookUpdate, Symbol};

/// A vertex identifier (index into the interned asset list).
pub type VertexId = u32;

/// Which side of a symbol a directed edge represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    /// `BASE -> QUOTE`, weighted by the fee-adjusted best bid.
    BaseToQuote,
    /// `QUOTE -> BASE`, weighted by the fee-adjusted inverse best ask.
    QuoteToBase,
}

/// A directed, weighted edge derived from one side of a symbol.
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    from: VertexId,
    to: VertexId,
    weight: f64,
    symbol: Symbol,
    side: EdgeSide,
}

impl Edge {
    /// Source vertex.
    #[must_use]
    pub fn from(&self) -> VertexId {
        self.from
    }
    /// Destination vertex.
    #[must_use]
    pub fn to(&self) -> VertexId {
        self.to
    }
    /// Current fee-adjusted rate. Zero means "no usable quote" (empty side or a
    /// stale book), which makes any containing cycle unprofitable.
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }
    /// The symbol this edge is derived from.
    #[must_use]
    pub fn symbol(&self) -> Symbol {
        self.symbol
    }
    /// Which side of the symbol this edge represents.
    #[must_use]
    pub fn side(&self) -> EdgeSide {
        self.side
    }
}

/// A precomputed directed 3-cycle: three edges forming a closed loop
/// `assets[0] -> assets[1] -> assets[2] -> assets[0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    /// Edge indices, in cycle order.
    pub edges: [usize; 3],
    /// Vertex ids, in cycle order.
    pub assets: [VertexId; 3],
}

#[derive(Debug, Clone, Copy)]
struct SymbolEdges {
    fwd: usize,
    rev: usize,
    fee: f64,
}

/// The currency graph: assets, fee-adjusted directed edges, precomputed
/// triangles, and a per-symbol affected-triangle index.
#[derive(Debug, Clone)]
pub struct AssetGraph {
    assets: Vec<Asset>,
    asset_index: HashMap<Asset, VertexId>,
    edges: Vec<Edge>,
    adjacency: Vec<Vec<VertexId>>,
    symbol_edges: HashMap<Symbol, SymbolEdges>,
    triangles: Vec<Triangle>,
    symbol_triangles: HashMap<Symbol, Vec<usize>>,
}

impl AssetGraph {
    /// Builds the graph from a set of `(symbol, taker_fee)` specifications. The
    /// fee is a fraction (for example `0.001` for 0.1%). Triangles are
    /// precomputed here.
    #[must_use]
    pub fn from_specs(specs: &[(Symbol, f64)]) -> AssetGraph {
        let mut assets: Vec<Asset> = Vec::new();
        let mut asset_index: HashMap<Asset, VertexId> = HashMap::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut edge_index: HashMap<(VertexId, VertexId), usize> = HashMap::new();
        let mut symbol_edges: HashMap<Symbol, SymbolEdges> = HashMap::new();

        for (symbol, fee) in specs {
            let base = intern(symbol.base(), &mut assets, &mut asset_index);
            let quote = intern(symbol.quote(), &mut assets, &mut asset_index);
            let fwd = add_edge(
                &mut edges,
                &mut edge_index,
                base,
                quote,
                *symbol,
                EdgeSide::BaseToQuote,
            );
            let rev = add_edge(
                &mut edges,
                &mut edge_index,
                quote,
                base,
                *symbol,
                EdgeSide::QuoteToBase,
            );
            symbol_edges.insert(
                *symbol,
                SymbolEdges {
                    fwd,
                    rev,
                    fee: *fee,
                },
            );
        }

        let mut adjacency: Vec<Vec<VertexId>> = vec![Vec::new(); assets.len()];
        for edge in &edges {
            adjacency[edge.from as usize].push(edge.to);
        }

        let triangles = enumerate_triangles(&adjacency, &edge_index);
        let symbol_triangles = index_symbol_triangles(&triangles, &edges);

        AssetGraph {
            assets,
            asset_index,
            edges,
            adjacency,
            symbol_edges,
            triangles,
            symbol_triangles,
        }
    }

    /// Number of assets (vertices).
    #[must_use]
    pub fn num_assets(&self) -> usize {
        self.assets.len()
    }

    /// Number of directed edges.
    #[must_use]
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Number of precomputed triangles.
    #[must_use]
    pub fn num_triangles(&self) -> usize {
        self.triangles.len()
    }

    /// The vertex id for an asset, if present.
    #[must_use]
    pub fn vertex(&self, asset: &Asset) -> Option<VertexId> {
        self.asset_index.get(asset).copied()
    }

    /// The asset for a vertex id.
    #[must_use]
    pub fn asset(&self, vertex: VertexId) -> Option<Asset> {
        self.assets.get(vertex as usize).copied()
    }

    /// All edges (for full-graph algorithms such as Bellman-Ford).
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Adjacency list (destination vertices per source vertex).
    #[must_use]
    pub fn adjacency(&self) -> &[Vec<VertexId>] {
        &self.adjacency
    }

    /// A single edge.
    #[must_use]
    pub fn edge(&self, id: usize) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// The current weight of an edge (0.0 if the id is out of range).
    #[must_use]
    pub fn edge_weight(&self, id: usize) -> f64 {
        self.edges.get(id).map_or(0.0, Edge::weight)
    }

    /// All precomputed triangles.
    #[must_use]
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    /// A single triangle.
    #[must_use]
    pub fn triangle(&self, id: usize) -> Option<&Triangle> {
        self.triangles.get(id)
    }

    /// The triangles that contain an edge of `symbol`. A book update for that
    /// symbol only needs these re-evaluated.
    #[must_use]
    pub fn affected_triangles(&self, symbol: &Symbol) -> &[usize] {
        self.symbol_triangles.get(symbol).map_or(&[], Vec::as_slice)
    }

    /// The product of a triangle's three edge weights. A value strictly greater
    /// than one is a (gross) profitable cycle. Returns 0.0 for an unknown id.
    #[must_use]
    pub fn triangle_product(&self, id: usize) -> f64 {
        match self.triangles.get(id) {
            Some(t) => {
                self.edge_weight(t.edges[0])
                    * self.edge_weight(t.edges[1])
                    * self.edge_weight(t.edges[2])
            }
            None => 0.0,
        }
    }

    /// The forward and reverse edge ids for a symbol, if known.
    #[must_use]
    pub fn symbol_edges(&self, symbol: &Symbol) -> Option<(usize, usize)> {
        self.symbol_edges.get(symbol).map(|se| (se.fwd, se.rev))
    }

    /// Updates the two edges derived from a symbol using a fresh book update.
    /// A stale book or an empty side yields a zero weight, which removes that
    /// edge's cycles from profitability until it recovers.
    pub fn update_from_book(&mut self, update: &BookUpdate) {
        let Some(se) = self.symbol_edges.get(&update.symbol).copied() else {
            return;
        };
        let net = 1.0 - se.fee;
        let live = update.status.is_live();

        let fwd_weight = if live {
            update.best_bid.map_or(0.0, |bid| bid.price.to_f64() * net)
        } else {
            0.0
        };
        let rev_weight = if live {
            update.best_ask.map_or(0.0, |ask| {
                let a = ask.price.to_f64();
                if a > 0.0 {
                    net / a
                } else {
                    0.0
                }
            })
        } else {
            0.0
        };

        self.edges[se.fwd].weight = fwd_weight;
        self.edges[se.rev].weight = rev_weight;
    }
}

fn intern(asset: Asset, assets: &mut Vec<Asset>, index: &mut HashMap<Asset, VertexId>) -> VertexId {
    if let Some(&id) = index.get(&asset) {
        return id;
    }
    let id = assets.len() as VertexId;
    assets.push(asset);
    index.insert(asset, id);
    id
}

fn add_edge(
    edges: &mut Vec<Edge>,
    edge_index: &mut HashMap<(VertexId, VertexId), usize>,
    from: VertexId,
    to: VertexId,
    symbol: Symbol,
    side: EdgeSide,
) -> usize {
    if let Some(&id) = edge_index.get(&(from, to)) {
        // A directed edge served by two symbols is unusual (it needs both
        // (A,B) and (B,A) to be listed markets). Last spec wins; we keep the id.
        edges[id].symbol = symbol;
        edges[id].side = side;
        return id;
    }
    let id = edges.len();
    edges.push(Edge {
        from,
        to,
        weight: 0.0,
        symbol,
        side,
    });
    edge_index.insert((from, to), id);
    id
}

/// Enumerates every directed 3-cycle exactly once. Rotations of the same cycle
/// are collapsed by keeping only the rotation whose first vertex is the
/// smallest; the two travel directions of a triangle are kept as distinct
/// cycles because they are distinct arbitrage opportunities.
fn enumerate_triangles(
    adjacency: &[Vec<VertexId>],
    edge_index: &HashMap<(VertexId, VertexId), usize>,
) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    for (a_idx, neighbors) in adjacency.iter().enumerate() {
        let a = a_idx as VertexId;
        for &b in neighbors {
            for &c in &adjacency[b as usize] {
                if a < b && a < c && b != c {
                    if let Some(&e2) = edge_index.get(&(c, a)) {
                        let e0 = edge_index[&(a, b)];
                        let e1 = edge_index[&(b, c)];
                        triangles.push(Triangle {
                            edges: [e0, e1, e2],
                            assets: [a, b, c],
                        });
                    }
                }
            }
        }
    }
    triangles
}

fn index_symbol_triangles(triangles: &[Triangle], edges: &[Edge]) -> HashMap<Symbol, Vec<usize>> {
    let mut map: HashMap<Symbol, Vec<usize>> = HashMap::new();
    for (tid, triangle) in triangles.iter().enumerate() {
        for &edge_id in &triangle.edges {
            let symbol = edges[edge_id].symbol;
            let entry = map.entry(symbol).or_default();
            if !entry.contains(&tid) {
                entry.push(tid);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{BookStatus, Decimal, Nanos, PriceLevel, SeqNo};

    fn sym(base: &str, quote: &str) -> Symbol {
        Symbol::from_parts(base, quote).unwrap()
    }

    fn triangle_specs() -> Vec<(Symbol, f64)> {
        vec![
            (sym("BTC", "USDT"), 0.001),
            (sym("ETH", "USDT"), 0.001),
            (sym("ETH", "BTC"), 0.001),
        ]
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

    #[test]
    fn builds_vertices_edges_and_triangles() {
        let g = AssetGraph::from_specs(&triangle_specs());
        assert_eq!(g.num_assets(), 3); // BTC, ETH, USDT
        assert_eq!(g.num_edges(), 6); // 2 per symbol
        assert_eq!(g.num_triangles(), 2); // the two travel directions
    }

    #[test]
    fn affected_triangles_cover_both_directions() {
        let g = AssetGraph::from_specs(&triangle_specs());
        // Every symbol participates in both directed triangles.
        assert_eq!(g.affected_triangles(&sym("BTC", "USDT")).len(), 2);
        assert_eq!(g.affected_triangles(&sym("ETH", "BTC")).len(), 2);
        assert!(g.affected_triangles(&sym("DOGE", "USDT")).is_empty());
    }

    #[test]
    fn weights_reflect_fee_and_side() {
        let mut g = AssetGraph::from_specs(&triangle_specs());
        g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60001"));
        let (fwd, rev) = g.symbol_edges(&sym("BTC", "USDT")).unwrap();
        // BASE->QUOTE = bid * (1 - fee) = 60000 * 0.999 = 59940.
        assert!((g.edge_weight(fwd) - 59_940.0).abs() < 1e-6);
        // QUOTE->BASE = (1 - fee) / ask = 0.999 / 60001.
        assert!((g.edge_weight(rev) - (0.999 / 60_001.0)).abs() < 1e-12);
    }

    #[test]
    fn stale_book_zeroes_weights() {
        let mut g = AssetGraph::from_specs(&triangle_specs());
        let mut b = book(sym("BTC", "USDT"), "60000", "60001");
        b.status = BookStatus::Stale;
        g.update_from_book(&b);
        let (fwd, rev) = g.symbol_edges(&sym("BTC", "USDT")).unwrap();
        assert_eq!(g.edge_weight(fwd), 0.0);
        assert_eq!(g.edge_weight(rev), 0.0);
    }

    #[test]
    fn detects_a_profitable_triangle() {
        // Construct a deliberate cross-rate arbitrage. With BTC/USDT ~ 60000 and
        // ETH/USDT ~ 3000, the fair ETH/BTC is 0.05. Quote ETH/BTC richer than
        // fair so a cycle clears above 1 before fees dominate. Fees set to 0 to
        // isolate the structure.
        let specs = vec![
            (sym("BTC", "USDT"), 0.0),
            (sym("ETH", "USDT"), 0.0),
            (sym("ETH", "BTC"), 0.0),
        ];
        let mut g = AssetGraph::from_specs(&specs);
        g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60000"));
        g.update_from_book(&book(sym("ETH", "USDT"), "3000", "3000"));
        // Fair ETH/BTC = 0.05; quote it at 0.052 (2 percent rich).
        g.update_from_book(&book(sym("ETH", "BTC"), "0.052", "0.052"));

        let best = (0..g.num_triangles())
            .map(|i| g.triangle_product(i))
            .fold(0.0_f64, f64::max);
        assert!(
            best > 1.0,
            "expected a profitable cycle, best product = {best}"
        );
    }

    #[test]
    fn zero_weight_makes_product_zero() {
        let g = AssetGraph::from_specs(&triangle_specs());
        // No book updates yet: all edges are zero, so no triangle is profitable.
        for i in 0..g.num_triangles() {
            assert_eq!(g.triangle_product(i), 0.0);
        }
    }
}
