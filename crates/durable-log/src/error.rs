//! Error types for durable-log.

use thiserror::Error;

/// Errors that can occur when using durable-log.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error from the underlying storage.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
