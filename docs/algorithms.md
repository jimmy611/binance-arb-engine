# Algorithms

The detection and evaluation mathematics, with the equivalences that justify the
implementation, complexity analysis, and worked examples. Cross-references point
at the code that implements each part.

## 1. The negative-cycle formulation

Model tradeable assets as vertices of a directed graph. For each pair
`(BASE, QUOTE)` add two directed edges:

- `BASE -> QUOTE` with weight `w = bid * (1 - fee)` (sell BASE for QUOTE),
- `QUOTE -> BASE` with weight `w = (1 - fee) / ask` (buy BASE with QUOTE).

A cycle `v0 -> v1 -> ... -> vk -> v0` converts one unit of `v0` into

    P = product(w_i)

units of `v0`. The cycle is profitable exactly when `P > 1`.

**Claim.** With the transform `c(e) = -ln(w(e))`, a cycle is profitable if and
only if it is a negative cycle under `c`.

**Proof.** `ln` is strictly increasing, so

    P > 1  <=>  ln(P) > 0  <=>  sum_i ln(w_i) > 0  <=>  sum_i (-ln(w_i)) < 0.

The right-hand side is the cycle's total cost under `c`, so `P > 1` iff the cycle
is negative. A zero-weight edge (an empty book side or a stale book) is dropped
from the cost graph (`ln 0` is undefined), which removes its cycles from
consideration, matching the intent that an untradeable leg makes its cycles
untradeable. QED.

This is why weights are `f64`: the transform and the sums are floating point, and
the relative error of a rate like `1/60000` is bounded and controlled. Amounts
(prices, quantities, notionals) remain exact fixed-point; see
`crates/core-types/src/decimal.rs`.

## 2. Algorithm A: Bellman-Ford negative-cycle detection

`crates/detector/src/bellman.rs`.

Initialize `dist[v] = 0` for every vertex (equivalent to a zero-cost virtual
source joined to all vertices, which detects a negative cycle anywhere in the
graph). Relax all edges `V` times. If no relaxation succeeds in a full pass, the
distances are final and there is no negative cycle. If a vertex `x` still relaxes
on the `V`-th pass, a negative cycle is reachable from `x`.

**Complexity.** `O(V * E)`: `V` passes over `E` edges.

**Correctness.** Without a negative cycle, shortest paths have at most `V - 1`
edges, so distances converge within `V - 1` passes and the `V`-th pass makes no
change. A change on the `V`-th pass therefore implies a negative cycle.

**Witness reconstruction.** From the vertex `x` updated on the last pass, follow
predecessors `V` times; the resulting vertex is guaranteed to lie on the cycle
(any prefix outside the cycle is consumed within `V` steps). Walk predecessors
from there until the start repeats to recover the loop, order the vertices so
consecutive pairs are joined by a real forward edge (looked up with
`AssetGraph::edge_between`), and read off the edges. The product of the edge
weights is the realized gross return factor.

**Role.** Cold start, periodic full-graph sanity checks against the fast path,
and cycles longer than three when enabled. It is not the hot path. Measured at
~8.6 us on a ~400-vertex, ~800-edge graph on a development laptop.

## 3. Algorithm C: the triangular fast path (default)

`crates/detector/src/triangle.rs`, `crates/graph/src/graph.rs`.

Triangles (directed 3-cycles) are precomputed once from the graph. Each rotation
of a cycle is collapsed to a single representative by keeping the rotation whose
first vertex is the smallest; the two travel directions are kept as distinct
opportunities. A per-symbol index maps a symbol to the triangles that contain one
of its edges.

Detecting profitability of one triangle is three multiplications and a compare
(`w0 * w1 * w2 > 1`).

- `scan_all` evaluates every triangle: `O(T)` for `T` triangles. Cold start and
  periodic sanity.
- `scan_affected` evaluates only the triangles that contain a just-updated
  symbol's edges: `O(k)` for the `k` affected triangles, typically a handful.
  This is Algorithm B (incremental re-evaluation) expressed over the triangle
  set, and it is the hot path. Measured at ~73 ns per update on a development
  laptop.

**Why this is optimal for triangles.** A single book update changes exactly two
edge weights (the two sides of one symbol). Only cycles containing those edges
can change profitability, so re-scanning just the affected triangles is both
sufficient and minimal.

## 4. Slippage-adjusted profitability (VWAP)

`crates/evaluator/src/vwap.rs`.

Top-of-book weights overstate what is tradeable: executing more than the top
quantity walks the book. For a cycle `A -> B -> C -> A` with starting notional
`n_A`, each leg converts its input into its output by consuming order-book levels
at monotonically worse prices.

Model leg `i` as a sequence of constant-rate chunks `(rate, capacity)`, one per
level, with `rate` decreasing (worse prices deeper in the book). For a sell leg
(consume bids) the rate is `price * (1 - fee)` (quote per base) and the capacity
is the level's base quantity; for a buy leg (consume asks) the rate is
`(1 - fee) / price` (base per quote) and the capacity is `price * quantity`
(quote).

**Concavity.** Each leg's output as a function of its input is concave and
increasing: it is piecewise linear with non-increasing slope (the chunk rates
decrease). A composition `f(g(x))` of concave increasing functions is concave and
increasing, so the cycle's output `out(n_A)` is concave and increasing, and the
profit `out(n_A) - n_A` is concave.

**Consequence.** A concave function's maximum over an interval is at a breakpoint
of its piecewise-linear derivative, i.e. at an order-book level boundary, not in
the interior. The composed marginal rate `r(n_A) = m1 * m2 * m3` (the product of
the three legs' current chunk rates) is a non-increasing step function of `n_A`.
Profit increases while `r >= 1` and decreases after, so the optimum is the total
size of all leading segments with `r >= 1`.

**Algorithm.** Advance `n_A` to the next boundary among the three legs (each leg's
remaining capacity expressed in units of A: `cap1`, `cap2 / m1`, `cap3 / (m1*m2)`),
emit a constant-rate segment, and repeat until a leg is exhausted. Then sum the
lengths of the leading `r >= 1` segments. This is exact and `O(depth)`; the
property tests in `crates/evaluator/tests/optimizer_props.rs` confirm both the
concavity (segment rates never increase) and that the reported optimum is the
true maximum over the feasible range.

### Worked example

Leg 1 sells A at rate 2 (one bid level, capacity 100 A). Leg 2 buys at rate 2
(deep). Leg 3 sells at rate 0.3 (deep). Composed rate `2 * 2 * 0.3 = 1.2 > 1`,
constant until leg 1 is exhausted at 100 A. Optimal size 100 A, profit
`0.2 * 100 = 20 A`. Add a worse second bid level to leg 1 (rate 0.3) and the
composed rate after the first 50 A drops to `0.3 * 2 * 0.3 = 0.18 < 1`; the
optimum is then 50 A. Both cases are unit tests.

## 5. Fee model

`crates/evaluator/src/fees.rs`. Tier-based maker/taker fees with per-symbol
overrides. Arbitrage uses taker fees by default (it requires immediacy). The fee
enters multiplicatively per leg as `(1 - fee)`, folded into the edge weights and
the VWAP chunk rates so all downstream profit is already net of fees.

## 6. Statistical confidence filter

`crates/evaluator/src/confidence.rs`. Three gates reject ghosts:

- **Persistence.** The cycle was profitable for at least `N` consecutive book
  updates (default 2).
- **Depth confidence.** The profitable top-of-book size is at least `K` times the
  minimum notional (default 1.5), so a small competing order does not erase it.
- **Latency-adjusted EV.** Raw profit is scaled by a survival probability that
  decays exponentially with round-trip latency,
  `p = base * 0.5^(latency / half_life)`, estimated from the empirical
  distribution of opportunity lifetimes measured during operation.

## 7. Position sizing: fractional Kelly

`crates/evaluator/src/kelly.rs`. For edge `e` (expected fractional return) and
variance `v`, the full-Kelly fraction is `e / v`. High-frequency parameter
estimates are noisy, so the size is scaled by a fractional multiplier (quarter
Kelly by default) and clamped to hard notional limits, then further capped by the
joint order-book capacity from Section 4:

    size = clamp((e / v) * multiplier * capital, min_notional, max_notional)
    size = min(size, order_book_capacity)

Zero edge, non-positive variance, or a capacity below the minimum notional means
"do not trade".
