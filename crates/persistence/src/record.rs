//! Write-ahead log record types.
//!
//! Every event that flows through the engine is wrapped in a [`WalRecord`] with a
//! monotonic sequence number and a wall-clock timestamp, then appended to the
//! WAL. Replaying records in sequence order reproduces a session exactly. The
//! payload enum grows as later milestones add event types (opportunities, risk
//! decisions, orders, fills); today it carries the market-data and order-book
//! events that already exist.

use serde::{Deserialize, Serialize};

use core_types::{BookUpdate, MarketEvent, Nanos, SeqNo};

/// The event carried by a WAL record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WalPayload {
    /// A normalized market event from an exchange adapter.
    Market(MarketEvent),
    /// An order-book update emitted by the book engine.
    Book(BookUpdate),
}

/// One durably logged event: sequence number, wall-clock time, and payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalRecord {
    /// Engine-assigned monotonic sequence number.
    pub seq: SeqNo,
    /// Wall-clock time the record was written.
    pub ts: Nanos,
    /// The event.
    pub payload: WalPayload,
}
