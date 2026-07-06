//! Error taxonomy shared across the engine.
//!
//! These are the low-level, value-construction errors that the type layer can
//! produce. Higher layers (order book resync, execution rejects, risk breaches)
//! define their own richer error types; this module stays deliberately small.

use thiserror::Error;

/// Errors produced when constructing or manipulating core value types.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// A decimal string could not be parsed into the fixed-point representation.
    #[error("invalid decimal {input:?}: {reason}")]
    ParseDecimal {
        /// The offending input, truncated for logs.
        input: String,
        /// Human-readable reason.
        reason: &'static str,
    },

    /// A fixed-point arithmetic operation overflowed the `i128` representation.
    ///
    /// On the hot path this is never allowed to panic silently: callers use the
    /// `checked_*` methods and treat `None` as a risk event.
    #[error("decimal overflow in {op}")]
    DecimalOverflow {
        /// The operation that overflowed (for example "mul" or "add").
        op: &'static str,
    },

    /// Division by zero in fixed-point arithmetic.
    #[error("decimal division by zero")]
    DecimalDivideByZero,

    /// An asset ticker was empty or exceeded the inline capacity.
    #[error("invalid asset {input:?}: {reason}")]
    InvalidAsset {
        /// The offending input.
        input: String,
        /// Human-readable reason.
        reason: &'static str,
    },
}

/// Convenience alias for results in this crate.
pub type Result<T> = core::result::Result<T, CoreError>;
