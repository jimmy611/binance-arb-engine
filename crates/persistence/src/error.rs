//! Persistence errors.

use thiserror::Error;

/// A failure reading or writing persisted state.
#[derive(Debug, Error)]
pub enum PersistError {
    /// An underlying IO error (file open, read, write).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A record could not be serialized or deserialized.
    #[error("serialization error: {0}")]
    Serde(#[from] bincode::Error),
}

/// Convenience result alias.
pub type Result<T> = core::result::Result<T, PersistError>;
