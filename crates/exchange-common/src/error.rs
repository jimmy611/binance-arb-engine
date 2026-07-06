//! Errors common to every exchange adapter.

use thiserror::Error;

/// A failure in an exchange adapter. Venue-specific error detail is flattened
/// into these variants with a human-readable message so the engine core never
/// depends on a particular venue's error types.
#[derive(Debug, Error)]
pub enum ExchangeError {
    /// The transport (WebSocket or HTTP connection) failed or dropped.
    #[error("connection error: {0}")]
    Connection(String),

    /// A protocol invariant was violated: a sequence gap, an out-of-order
    /// update, or an unexpected message shape. For order books this is fatal
    /// for the affected symbol until resync.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// A payload could not be parsed into the expected shape.
    #[error("parse error: {0}")]
    Parse(String),

    /// An HTTP request returned a non-success status.
    #[error("http {status}: {body}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Response body, truncated for logs.
        body: String,
    },

    /// A rate limit was hit. The engine must back off; the token buckets exist
    /// to prevent ever reaching this in normal operation.
    #[error("rate limited (retry after {retry_after_ms:?} ms)")]
    RateLimited {
        /// Suggested backoff in milliseconds, if the venue provided one.
        retry_after_ms: Option<u64>,
    },

    /// An operation timed out.
    #[error("timeout")]
    Timeout,

    /// The operation is not supported by this adapter in its current build.
    /// Used, for example, by order submission in a read-only market-data build
    /// before the execution milestone. This is an explicit, logged refusal, not
    /// a silent no-op.
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

/// Convenience result alias for adapter operations.
pub type Result<T> = core::result::Result<T, ExchangeError>;
