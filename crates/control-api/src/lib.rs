//! Control plane: gRPC and WebSocket API for the frontend.
//!
//! Built at M8. Serves the services defined in `proto/` (EngineControl,
//! MarketData, OpportunityFeed, TradeStream, Metrics, History, Config) over
//! gRPC, and a single multiplexed WebSocket for the streaming topics (ticker,
//! opportunities, trades, metrics, alerts). Reads hot state from Redis and
//! history from TimescaleDB; writes config back over a versioned, optimistically
//! locked control channel. The `.proto` files are the single source of truth;
//! Rust server stubs are generated from them at build time.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
