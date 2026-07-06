# Algorithms

Expanded write-up of the detection and evaluation algorithms, with proofs,
complexity analysis, and worked examples. This document is authored in full at
milestone M5 (detector) and M6 (evaluator), alongside the code it describes, so
that the proofs match the implementation exactly rather than being written ahead
of it.

Planned contents:

- The negative-cycle formulation: why `product(w_i) > 1` is equivalent to a
  negative cycle under the `w -> -log(w)` transform, with the fee adjustment.
- Algorithm C, triangle fast path: precomputation, per-update affected-triangle
  enumeration, complexity, and why it is the default.
- Algorithm A, Bellman-Ford: negative-cycle detection in O(V*E), witness
  reconstruction, and its role in cold start and periodic sanity checks.
- Algorithm B, incremental re-evaluation: the per-edge cycle membership index
  and the bounded re-check on a single edge update.
- Slippage-adjusted profitability: the concave piecewise-linear profit function
  in starting notional, and why evaluating order-book breakpoints yields the
  exact optimum in O(depth).
- The fee model, the statistical confidence filter, and fractional Kelly sizing.
