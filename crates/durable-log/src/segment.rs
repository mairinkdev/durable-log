//! Segment naming and discovery.
//!
//! Segments are named `segment_{base_offset}.log` with zero-padded `base_offset`
//! so that lexicographic order matches numeric order.

use crate::Result;
use std::path::{Path, PathBuf};

/// Filename prefix for segment data files.
const SEGMENT_LOG_PREFIX: &str = "segment_";
/// Filename suffix for segment data files.
const SEGMENT_LOG_SUFFIX: &str = ".log";
/// Base offset is zero-padded to this width for sortable filenames.
const BASE_OFFSET_WIDTH: u32 = 20;

/// Identifies a segment by its base offset (first record offset in the segment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentId(pub u64);

impl SegmentId {
    /// Builds the segment data filename for this base offset.
    #[must_use]
    pub fn log_filename(&self) -> String {
        format!(
            "{SEGMENT_LOG_PREFIX}{:0>width$}{SEGMENT_LOG_SUFFIX}",
            self.0,
            width = BASE_OFFSET_WIDTH as usize
        )
    }

    /// Parses a segment id from a log filename (e.g. `segment_00000000000000000001.log`).
    /// Returns `None` if the filename does not match the pattern.
    #[must_use]
    pub fn from_log_filename(name: &str) -> Option<Self> {
        let stem = name
            .strip_prefix(SEGMENT_LOG_PREFIX)?
            .strip_suffix(SEGMENT_LOG_SUFFIX)?;
        stem.parse::<u64>().ok().map(Self)
    }
}

/// Info for a discovered segment (base offset and full path to the .log file).
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    /// Base offset of this segment.
    pub base_offset: u64,
    /// Full path to the segment's .log file.
    pub log_path: PathBuf,
}

/// Discovers all segment log files in `dir`, sorted by base offset ascending.
///
/// # Errors
///
/// Returns I/O errors from reading the directory.
pub fn discover_segments(dir: &Path) -> Result<Vec<SegmentInfo>> {
    let mut segments = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(crate::Error::from)?;
    for entry in entries {
        let entry = entry.map_err(crate::Error::from)?;
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(id) = SegmentId::from_log_filename(name) {
                    segments.push(SegmentInfo {
                        base_offset: id.0,
                        log_path: path,
                    });
                }
            }
        }
    }
    segments.sort_by_key(|s| s.base_offset);
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_id_filename_roundtrip() {
        let id = SegmentId(0);
        assert_eq!(SegmentId::from_log_filename(&id.log_filename()), Some(id));
        let id = SegmentId(1);
        assert_eq!(SegmentId::from_log_filename(&id.log_filename()), Some(id));
        let id = SegmentId(123_456_789);
        assert_eq!(SegmentId::from_log_filename(&id.log_filename()), Some(id));
    }

    #[test]
    fn segment_filename_sort_order() {
        let a = SegmentId(1).log_filename();
        let b = SegmentId(2).log_filename();
        let c = SegmentId(10).log_filename();
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn from_log_filename_rejects_invalid() {
        assert!(SegmentId::from_log_filename("other.log").is_none());
        assert!(SegmentId::from_log_filename("segment_abc.log").is_none());
        assert!(SegmentId::from_log_filename("segment_1.dat").is_none());
    }
}
