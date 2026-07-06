//! In-memory L2 order book with Binance delta application and resync.
//!
//! Two layers:
//!   * [`L2Book`]: the pure price ladder (apply snapshot/diff, read top of book).
//!     No sequencing logic, fully deterministic.
//!   * [`SymbolBook`] and [`BookManager`]: the resync state machine that follows
//!     Binance's exact `U`/`u` continuity protocol, buffers diffs before a
//!     snapshot, marks a book stale on any gap, and stamps every emitted
//!     [`core_types::BookUpdate`] with a monotonic sequence number.
//!
//! The manager is fed [`core_types::DepthDelta`] and [`core_types::DepthSnapshot`]
//! events (produced by an exchange adapter) and returns [`BookEvent`]s telling
//! the caller what happened, including when a fresh snapshot must be fetched.
#![forbid(unsafe_code)]

pub mod book;
pub mod sync;

pub use book::L2Book;
pub use sync::{BookEvent, BookManager, SymbolBook};
