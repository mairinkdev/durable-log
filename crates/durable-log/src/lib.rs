//! # durable-log
//!
//! Crash-safe, segmented commit log (WAL) with checksums and index.
//!
//! See [README](https://github.com/your-org/durable-log#readme) for overview and examples.

pub mod error;
pub mod log;
pub mod log_dir;
pub mod record;
pub mod segment;

pub use error::Error;
pub use log::{Config, Log};
pub use log_dir::LogDir;
pub use record::{decode_record, encode_record, RecordHeader, HEADER_LEN, MAGIC, VERSION_V1};
pub use segment::{discover_segments, SegmentId, SegmentInfo};

/// Result type for durable-log operations.
pub type Result<T> = std::result::Result<T, Error>;
