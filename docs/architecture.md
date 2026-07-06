# Architecture

This document is the living version of the build specification, refined during
implementation. It records what the system is, the decisions taken and why, the
places where the original specification was factually corrected, and the phased
plan with current status.

## 1. What this is

A low-latency triangular arbitrage engine for Binance spot, written in Rust,
with a React trading-terminal frontend. It continuously maintains order books
for the traded symbol universe, models the assets as a currency graph, detects
profitable cycles (product of fee-adjusted edge weights greater than one,
equivalently a negative cycle under the negative-log transform), evaluates each
candidate for post-fee and post-slippage expected value, and either executes
autonomously above a configured threshold or surfaces it for review. Binance is
one implementation of an `ExchangeAdapter` so that cross-exchange inventory
arbitrage can be added later without a rewrite.

The engine boots in dry-run. Live trading is gated behind a signed config flag,
a UI confirmation, and a passing 24-hour backtest.

## 2. Decisions and corrections

These are the points where implementation judgement diverged from, or sharpened,
the original specification. They are recorded so the reasoning is not lost.

### 2.1 Colocation and the latency target

The specification described placing the engine in an AWS `cluster` placement
group "colocated with Binance's matching engine." That is not achievable: an EC2
placement group only groups instances within your own account and VPC, and there
is no colo cross-connect to Binance's matching engine for API users. What is real
and worthwhile is running in `ap-northeast-1` (Tokyo), where Binance spot
infrastructure is widely reported to live, with warm keep-alive TLS to minimise
round-trip time to Binance's public API gateway. The sub-25ms decision-to-ack
target is therefore an empirical goal to be measured, gated by Binance's
published rate limits, not a guarantee delivered by proximity. The Terraform
module documents this honestly.

### 2.2 Live profitability expectation

Retail triangular arbitrage through the public Binance API competes with faster,
lower-fee participants, and the pure-triangular edge is thin and short-lived. A
negative backtest is a normal, expected outcome, not a defect. The dry-run
default and the backtest gate exist precisely so that live trading is never
enabled on a strategy that has not demonstrated positive expected value on real
recorded order flow. No parameter is tuned to make a backtest look good.

### 2.3 "Atomic" execution is not atomic

Execution mode 1 submits three independent orders in parallel. There is no
cross-order atomicity: IOC legs can partial-fill and strand inventory. The code
names this mode "parallel," not "atomic," and treats the leg-risk detection and
market-order unwind path as first-class rather than an afterthought.

### 2.4 Numeric representation

The specification permits `rust_decimal` "or equivalent" for money and `f64`
only in log-space detection. The chosen equivalent is a fixed-point `Decimal`
backed by an `i128` scaled by `10^8` (see `crates/core-types/src/decimal.rs`).
Reasons: `rust_decimal` is far too slow for the microsecond budget when many
triangles are re-evaluated per update; scaled-`i128` arithmetic is exact and
fast (measured ~31ns per checked multiply or divide, ~2ns per add on a 2019 x86
laptop). Eight fractional digits match Binance's maximum spot quoting precision,
so no exchange value loses information on ingest. The split is strict: amounts
(prices, quantities, notionals) are exact `Decimal`; exchange rates and
log-weights are `f64`, held inside the detector where relative error is bounded
and controlled.

## 3. Components and data flow

Data flows in one direction through loosely coupled components communicating over
lock-free channels within a single process. Module boundaries are drawn so a
component can later be split across processes or hosts.

1. Exchange Connector (`exchange-binance`): WebSocket lifecycle, auth,
   subscriptions, sequence validation, reconnect with backoff and jitter.
2. Market Data Normalizer (`exchange-binance` + `exchange-common`): venue
   payloads to the unified `MarketEvent`.
3. Order Book Engine (`orderbook`): per-symbol L2 book, snapshot and diff
   application following Binance's exact `U`/`u` protocol, `BookUpdate` with a
   monotonic sequence number, stale-on-gap.
4. Currency Graph (`graph`): assets as vertices, fee-adjusted directed edges,
   triangle precomputation, incremental edge updates.
5. Arbitrage Detector (`detector`): triangle fast path, Bellman-Ford,
   incremental re-evaluation.
6. Opportunity Evaluator (`evaluator`): fees, VWAP slippage optimizer, expected
   value, confidence filter, fractional Kelly sizing.
7. Execution Router (`execution`): mode selection, Ed25519 signing, IOC/market
   routing, WS-API session.
8. Position and PnL Tracker (`execution`/`engine`): user-data reconciliation,
   realized and unrealized PnL, inventory.
9. Risk Manager (`risk`): limits, circuit breakers, three-channel kill switch.
10. Persistence Layer (`persistence`): WAL, TimescaleDB, Redis, non-blocking.
11. Control Plane / API Gateway (`control-api`): gRPC and WebSocket for the
    frontend.

The `engine` crate wires these together; `replay` and `backtest` drive the same
component code offline.

## 4. Hot-path latency budget

Event ingress to order egress, single host. Each component owns a slice and is
held to it by the criterion latency gate in CI. These are targets to measure
against, not achieved figures; the c7i production host is the reference, not a
development laptop.

| Stage                                   | Budget (us) |
| --------------------------------------- | ----------: |
| WebSocket parse and normalize           |          20 |
| Order book apply                        |          15 |
| Graph edge update + affected triangles  |          50 |
| Detection (bounded triangle check)      |         200 |
| Evaluation (fees, slippage, EV, risk)   |         100 |
| Order marshaling and signing (Ed25519)  |          80 |
| Kernel and NIC egress                   |          35 |
| Total                                   |         500 |

## 5. Numeric layer (implemented, M1)

`core-types` is the shared vocabulary and has no dependency on any other
workspace crate, so it can never introduce a cycle.

- `Decimal`: fixed-point `i128` at scale `10^8`, checked arithmetic with
  round-half-away-from-zero re-scaling, exact string round-trip, `f64` escape
  hatch for the detector only.
- `Asset` / `RawSymbol`: `Copy`, inline, allocation-free ASCII identifiers so
  naming a symbol on the hot path never allocates.
- `Symbol`: base, quote, and authoritative raw exchange id.
- `MarketEvent`: `BookTicker`, `DepthSnapshot`, `DepthDelta`, `Trade`.
- `BookUpdate` / `BookStatus`: the order book's output contract.
- `SymbolFilters`: exchange trading rules (tick, step, min qty, min notional).
- `SeqNo`, `OrderId`, `ClientOrderId`, `Nanos`, `EngineMode`.

## 6. Implementation plan and status

Built in milestones. Each milestone ends in code that compiles, passes tests and
clippy (`-D warnings`), and is formatted. No hot-path logic is stubbed.

- [x] M0 Workspace scaffold, CI, proto/buf config, compose and terraform
  skeletons.
- [x] M1 `core-types` with property tests and a criterion bench.
- [x] M2 `exchange-binance` read-only market data: combined-stream WebSocket
  with jittered backoff, REST client (exchangeInfo, depth snapshot) with a
  weight token bucket, the exchangeInfo symbol table, and a pure normalizer
  fixture-tested against recorded live frames. The depth resync state machine
  itself lands with the book at M3.
- [x] M3 `orderbook`: the pure `L2Book` price ladder plus the `SymbolBook`
  resync state machine (buffer diffs before snapshot, `U`/`u` continuity,
  stale-on-gap, resync recovery) and a `BookManager` that stamps every
  `BookUpdate` with a monotonic sequence number. Property tests assert the book
  is never crossed, contiguous diffs advance the update id, and sequence numbers
  strictly increase.
- [x] M4 `graph`: asset interner, two fee-adjusted directed edges per symbol
  (best bid, inverse best ask), precomputed directed triangles (both travel
  directions), a per-symbol affected-triangle index for incremental
  re-evaluation, and `update_from_book` that zeroes weights on a stale book.
- [x] M5 `detector`: Algorithm C triangle fast path (`scan_all` for cold start,
  `scan_affected` for the incremental hot path, Algorithm B), and Algorithm A
  Bellman-Ford negative-cycle detection over `-ln(weight)` with witness-cycle
  reconstruction for cold start, periodic sanity, and longer cycles. Criterion
  benches gate the hot path (measured on a dev laptop: `scan_affected` ~73 ns,
  `scan_all` over 400 triangles ~1.36 us, Bellman-Ford full graph ~8.6 us).
  Detection produces candidate cycles only; slippage, EV, and sizing are M6.
- [x] M6 `evaluator`: the VWAP breakpoint optimizer (composes three legs'
  order-book depth into a concave, piecewise-linear profit shape and finds the
  exact profit-maximizing size in O(depth)), the tier-based fee schedule with
  per-symbol overrides, the confidence filter (persistence, depth confidence,
  latency-adjusted survival EV), and fractional Kelly sizing capped by hard
  limits and order-book capacity. Property tests confirm the profit shape is
  concave and the reported optimum is the true maximum.
- [~] M7 `persistence`: the append-only WAL (length-prefixed bincode frames,
  daily UTC-midnight rotation via a dependency-free civil-date calc, a reader
  that stops cleanly at a torn final frame) is built and tested, along with the
  `HotState` trait, an in-memory implementation, and the TimescaleDB schema. The
  Redis-backed `HotState`, the TimescaleDB writer, and the `replay` binary are
  deferred until those services can be run locally (docker-compose is authored);
  they are documented, not stubbed with fake behavior. This required switching
  `MarketEvent` to serde's externally tagged representation so it survives a
  non-self-describing binary format.
- [x] M8 `control-api`: the full protobuf contract in `proto/` (EngineControl,
  Config, History, and the streaming BookTop/Opportunity/Trade/SystemMetrics
  types), generated to Rust via tonic-build with serde derived so the same types
  serialize to JSON over the WebSocket. gRPC services (EngineControl, Config,
  History) run over a shared `ControlState` with versioned optimistic-locking
  config, a live-mode confirmation-phrase gate, and an emergency stop that forces
  dry-run. Streaming is a tokio broadcast `Hub` fanned out over a multiplexed
  `serve_ws` socket.
- [x] M9 `frontend`: the React/TypeScript/Vite/Tailwind trading terminal. Dense
  dark aesthetic with glassmorphism chrome and high-contrast, tabular-numeric
  data surfaces; light/dark themes. Six screens (Dashboard, Opportunities with a
  virtualized feed, Trades, Risk, Backtest, Settings), Zustand state, TanStack
  Query, lightweight-charts (PnL) and Recharts (latency), a three-channel kill
  switch, and a live-mode confirmation gate. A `Transport` seam runs a built-in
  market simulation by default (so every screen is live without a backend) and
  swaps to the M8 WebSocket hub + gRPC/JSON control when `VITE_STREAM_WS` is set.
  Typecheck, lint, and production build are green in CI.
- [x] M10 `execution` dry-run: order types and the `OrderExecution` capability
  trait (`exchange-common`), Ed25519 request signing in the Binance adapter
  (signature verified against the public key in tests), execution-mode selection
  (parallel IOC vs sequential lock-in vs dry-run), filter-aware order building
  that snaps quantities to the lot step and prices to the tick and flags plans
  that breach the minimum notional, and a router that in dry-run logs every
  would-be order and submits nothing. Live submission is M12.
- [x] M11 `risk`: the `RiskManager` enforces hard limits before every execution
  (per-trade, total, and per-asset notional; rolling trade-rate; daily drawdown;
  opportunity age), tracks exposure and realized PnL, trips a circuit breaker on
  consecutive or windowed execution failures (forcing safe dry-run mode), and
  owns a single kill switch shared by three channels (UI, CLI, HTTP) as an
  `Arc<AtomicBool>`. Soft limits (ghost/reject/slippage rates, sequence gaps)
  raise warnings without vetoing. Eleven unit tests cover each path.
- [x] M12 `execution` live path: the live state machine (`execute_live`) is
  generic over an `OrderExecution` adapter and handles both modes (parallel IOC,
  sequential lock-in), partial fills, rejects, and inventory unwind with opposite
  market orders; it is verified against a mock adapter across every branch. The
  real Binance trader adapter (`BinanceTrader`) builds Ed25519-signed order
  submit/cancel requests and is code-complete but not exercised live: sending
  real orders needs registered keys and is a real-money action requiring operator
  setup and confirmation, so it is gated behind the signed live-mode flag.
- [~] M13 (partial): the `engine` binary is the capstone that wires the full
  dry-run pipeline (feed -> book -> graph -> detector -> evaluator -> risk ->
  dry-run) with the WAL, the streaming hub, and the control API, and was verified
  live against Binance read-only market data (780 book updates processed, 0
  profitable cycles after fees, the expected honest outcome). `docs/algorithms.md`
  is written with the negative-cycle proof, complexity analysis, the VWAP
  concavity argument, and worked examples. Deferred until the required
  infrastructure or credentials are available: the full Prometheus/OpenTelemetry
  observability stack, the Redis and TimescaleDB clients, the `replay` and
  `backtest` binaries and the 24-hour backtest report (need recorded data and a
  running engine), the engine Dockerfile, and a live `terraform apply`. The
  docker-compose services, the TimescaleDB schema, and the Terraform skeleton are
  authored and committed.

## 7. Testing and CI

- Unit tests for every pure function; `proptest` for order book and decimal
  invariants.
- Integration tests per adapter against recorded WAL streams.
- Full-pipeline replay tests over recorded data.
- Criterion latency gate: a PR that regresses a hot-path benchmark beyond
  threshold fails CI.
- Chaos tests: disconnects, delayed acks, out-of-order sequences, malformed
  messages.
- `cargo deny` for supply-chain hygiene.
