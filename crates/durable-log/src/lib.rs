//! # durable-log
//!
//! Crash-safe, segmented commit log (WAL) with checksums and index.
//!
//! See [README](https://github.com/your-org/durable-log#readme) for overview and examples.

pub mod error;

pub use error::Error;

/// Result type for durable-log operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_test() {}
}
