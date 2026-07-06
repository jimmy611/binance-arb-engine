//! In-memory L2 order book with delta application and validation.
//!
//! Built at M3. Maintains a per-symbol book, applies REST snapshots and
//! WebSocket diffs following Binance's exact `U`/`u` sequence protocol, marks a
//! symbol stale on any sequence gap until resync completes, and emits
//! `BookUpdate`s with a monotonically increasing sequence number. Property
//! tests assert the invariants: best bid never exceeds best ask, quantity is
//! conserved under delta application, and sequence numbers are monotonic.
//!
//! Deliberately empty at milestone M0.
#![forbid(unsafe_code)]
