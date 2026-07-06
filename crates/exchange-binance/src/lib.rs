//! Binance adapter: combined-stream WebSocket market data, REST snapshots, and
//! the WS-API order path.
//!
//! Responsibilities (built at M2 for read-only market data, extended at M10/M12
//! for order submission):
//!   * Manage combined-stream WebSocket connections with reconnect, exponential
//!     backoff, and jitter.
//!   * Fetch REST depth snapshots and drive the documented buffer-then-apply
//!     resync protocol.
//!   * Normalize Binance payloads into [`core_types::MarketEvent`].
//!   * Track REST weight and order-rate token buckets, never exceeding 80% of
//!     any declared limit.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
