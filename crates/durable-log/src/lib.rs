//! # durable-log
//!
//! Crash-safe, segmented commit log (WAL) with checksums and index.
//!
//! See [README](https://github.com/your-org/durable-log#readme) for overview and examples.

pub mod error;
pub mod record;

pub use error::Error;
pub use record::{decode_record, encode_record, RecordHeader, HEADER_LEN, MAGIC, VERSION_V1};

/// Result type for durable-log operations.
pub type Result<T> = std::result::Result<T, Error>;
