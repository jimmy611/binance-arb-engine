//! Persistence: append-only WAL, TimescaleDB time-series, and Redis hot state.
//!
//! Built at M7. Serializes every event (MarketEvent, BookUpdate, Opportunity,
//! RiskDecision, OrderRequest, OrderResponse, Fill) into length-prefixed frames
//! in a daily-rotated write-ahead log with a monotonic sequence number and a
//! wall-clock timestamp, enabling deterministic replay. Writes aggregates to
//! TimescaleDB and hot state to Redis. All writes are non-blocking: the hot
//! path never awaits a disk or network write.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
