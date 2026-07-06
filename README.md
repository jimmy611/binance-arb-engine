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
- [x] **M7 (core)** `persistence`: append-only WAL (bincode frames, daily UTC rotation, torn-frame-tolerant reader) as the deterministic-replay foundation, `HotState` trait + in-memory store, TimescaleDB schema. Redis and TimescaleDB clients and the `replay` binary are deferred until that infra can be exercised.
- [x] **M8** `control-api`: full protobuf contract (single source of truth) with tonic codegen + serde derives; `EngineControl`/`Config`/`History` gRPC services over versioned `ControlState` (optimistic locking, live-mode confirmation gate, emergency stop); multiplexed WebSocket streaming hub
- [x] **M9** `frontend`: React 18 + TS + Vite + Tailwind trading terminal (glassmorphism chrome, high-contrast data), six screens (Dashboard, Opportunities, Trades, Risk, Backtest, Settings), Zustand + TanStack Query, lightweight-charts + Recharts, virtualized opportunity feed, three-channel kill switch. `Transport` seam: simulation feed by default, real WebSocket/gRPC adapter for the M8 backend. Typecheck + lint + build green.
- [x] **M10** `execution` (dry-run): order types + `OrderExecution` trait, Ed25519 request signing (verified), execution-mode selection (parallel IOC / sequential lock-in / dry-run), filter-aware order building (snap to tick/step, min-notional check), dry-run router that logs would-be orders and sends nothing
- [x] **M11** `risk`: pre-trade hard-limit checks (per-trade / total / per-asset notional, trade rate, daily drawdown, opportunity age), exposure tracking, circuit breaker (consecutive/windowed failures force safe mode), soft-limit warnings, and a shared three-channel kill switch
- [x] **M12** `execution` (live path): the live state machine (parallel IOC / sequential lock-in) with partial-fill and reject handling and inventory unwind, generic over `OrderExecution` and tested against a mock; plus the real Binance trader adapter (Ed25519-signed order submit/cancel), code-complete. No live orders are sent in this environment (no keys; real-money action requires operator setup).
- [~] **M13** (partial): `engine` capstone wires the full dry-run pipeline end to end (verified live read-only: 780 book updates, 0 profitable cycles after fees); `docs/algorithms.md` written with proofs and complexity. Deferred (infra/keys): full Prometheus/OpenTelemetry stack, Redis/TimescaleDB clients, `replay`/`backtest` binaries and the 24h report, engine Dockerfile, Terraform apply. `docker-compose.yml`, the TimescaleDB schema, and the Terraform skeleton are authored.

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
