//! Persistence: the write-ahead log, the hot-state cache, and the time-series
//! sink.
//!
//! Implemented and tested here:
//!   * [`wal`]: the append-only, daily-rotated write-ahead log and its reader.
//!     This is the deterministic-replay foundation: every engine event is logged
//!     with a monotonic sequence number, and replaying the log reproduces a
//!     session bit-for-bit.
//!   * [`hotstate`]: the [`HotState`] trait and an in-memory implementation for
//!     current top-of-book and engine mode.
//!
//! Deferred until their services can be exercised (see `docker-compose.yml` and
//! `migrations/`): the Redis-backed [`HotState`] and the TimescaleDB time-series
//! sink. Their schema and interfaces are defined; the concrete clients land when
//! the infrastructure is available rather than being stubbed with fake behavior.
//! All persistence writes are designed to be non-blocking: the hot path enqueues
//! to a channel and a dedicated writer drains it, so it never awaits a disk or
//! network write.
#![forbid(unsafe_code)]

pub mod error;
pub mod hotstate;
pub mod record;
pub mod wal;

pub use error::{PersistError, Result};
pub use hotstate::{HotState, InMemoryHotState};
pub use record::{WalPayload, WalRecord};
pub use wal::{day_index, wal_filename, WalReader, WalWriter};
