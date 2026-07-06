# ARB: Real-Time Triangular Arbitrage Engine (Binance)

A production-oriented, low-latency triangular arbitrage engine for Binance spot,
built in Rust with a React/TypeScript trading terminal frontend. Structured so
that Binance is one implementation of an `ExchangeAdapter` and cross-exchange
inventory arbitrage can be added later without a rewrite.

The engine starts in **dry-run (paper) mode**. Live order submission is gated
behind an explicit signed configuration flag, a UI confirmation, and a passing
24-hour backtest that shows positive expected value at the configured risk
parameters. See `docs/runbook.md` before touching live mode.

> Reality check for operators: retail triangular arbitrage through the public
> Binance API competes with faster, lower-fee participants. A negative backtest
> is a normal, expected outcome, not a bug. The dry-run default and the
> backtest gate exist precisely so that live trading is never enabled on a
> strategy that has not demonstrated positive expected value on real recorded
> order flow. Do not tune parameters to make a backtest look good.

## Status

This repository is being built in milestones (see `docs/architecture.md`,
section "Implementation Plan"). Current state:

- [x] **M0** Workspace scaffold, CI, proto/buf config, compose + terraform skeletons
- [x] **M1** `core-types`: fixed-point decimal, symbol/asset, market events, book/id/time/error types
- [x] **M2** `exchange-binance` (read-only): combined-stream WebSocket, REST snapshot, symbol table, normalizer, rate limiter, backoff
- [x] **M3** `orderbook`: L2 price ladder + Binance `U`/`u` resync state machine (buffer, stale-on-gap), monotonic `BookUpdate`s, property tests
- [x] **M4** `graph`: asset interner, fee-adjusted directed edges (bid / inverse-ask), triangle precompute, per-symbol affected-triangle index, incremental updates
- [x] **M5** `detector`: triangle fast path (Algorithm C) + incremental affected scan (B) + Bellman-Ford negative-cycle detection (A) with witness reconstruction; criterion latency gate (scan_affected ~73 ns, Bellman-Ford ~8.6 µs on a dev laptop)
- [x] **M6** `evaluator`: VWAP concave breakpoint optimizer (exact profit-max size), fee schedule, confidence filter (persistence/depth/survival EV), fractional Kelly sizing; property tests for concavity
- [ ] M7 `persistence` + `replay`
- [ ] M8 `control-api` (gRPC + WebSocket)
- [ ] M9 frontend
- [ ] M10 `execution` (dry-run)
- [ ] M11 `risk` (limits, circuit breakers, kill switch)
- [ ] M12 `execution` (live)
- [ ] M13 `backtest`, observability, docs, deployment

## Layout

```
crates/
  core-types/       Fixed-point decimal, Symbol/Asset, MarketEvent, order-book types, IDs, errors
  exchange-common/  ExchangeAdapter trait, shared normalizers
  exchange-binance/ Binance WebSocket + REST + WS-API adapter
  orderbook/        L2 order book with delta application and validation
  graph/            Currency graph, edge updates, triangle precomputation
  detector/         Bellman-Ford, incremental scanner, triangle fast path
  evaluator/        Fees, slippage, EV, Kelly sizing, confidence filter
  execution/        Order signing, IOC/market routing, mode selection
  risk/             Limit enforcement, circuit breakers, kill switch
  persistence/      WAL, TimescaleDB, Redis
  control-api/      gRPC + WebSocket server for the frontend
  engine/           Wires everything, main binary
  replay/           Offline replay harness
  backtest/         Historical simulation
proto/              Protobuf API contract (single source of truth for the API)
frontend/           React + TypeScript trading terminal (lands at M9)
terraform/          AWS ap-northeast-1 production module (skeleton)
docs/               architecture, algorithms, runbook, backtest guides
```

## Quick start (development)

Prerequisites: Rust stable (>= 1.96), `protoc`, Node 22+ (for the frontend),
and optionally Docker for Redis and TimescaleDB.

```bash
# Build and test the whole workspace.
cargo build --workspace
cargo test  --workspace

# Lint exactly as CI does.
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Run the hot-path microbenchmarks.
cargo bench -p core-types
```

Local infrastructure (Redis + TimescaleDB) will be provided by
`docker-compose.yml` and consumed starting at milestone M7.

## License

MIT OR Apache-2.0.
