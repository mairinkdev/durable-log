//! Log directory open and exclusive writer lock.

use crate::error::Error;
use crate::segment::{discover_segments, SegmentInfo};
use crate::Result;
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

/// Name of the lock file used to ensure a single writer per log directory.
const LOCK_FILE_NAME: &str = "write.lock";

/// An open log directory with exclusive write lock held.
///
/// Creating a `LogDir` acquires an OS-level exclusive lock on `write.lock`.
/// Only one `LogDir` (per process or per machine, depending on OS) can exist
/// for a given path at a time. Drop releases the lock.
#[derive(Debug)]
pub struct LogDir {
    /// Root path of the log directory.
    path: PathBuf,
    /// Lock file held open for the duration; lock is released on drop.
    _lock: File,
    /// Discovered segments sorted by base offset.
    segments: Vec<SegmentInfo>,
}

impl LogDir {
    /// Opens the log directory at `path`, creating it if it does not exist,
    /// and acquires the exclusive writer lock. Discovers existing segments.
    ///
    /// # Errors
    ///
    /// - I/O errors when creating the directory or reading it.
    /// - [`Error::Locked`] if the lock is already held (e.g. another process or holder).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path).map_err(Error::from)?;

        let lock_path = path.join(LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(Error::from)?;

        file.try_lock_exclusive().map_err(|e| {
            Error::Locked(format!(
                "log directory is already locked (single writer required): {e}"
            ))
        })?;

        let segments = discover_segments(&path)?;

        Ok(Self {
            path,
            _lock: file,
            segments,
        })
    }

    /// Returns the root path of the log directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns discovered segments in order of base offset (ascending).
    #[must_use]
    pub fn segments(&self) -> &[SegmentInfo] {
        &self.segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_dir_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let log_dir = LogDir::open(dir.path()).unwrap();
        assert_eq!(log_dir.path(), dir.path());
        assert!(log_dir.segments().is_empty());
    }

    #[test]
    fn open_creates_dir_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("nonexistent");
        let log_dir = LogDir::open(&sub).unwrap();
        assert!(sub.exists());
        assert!(sub.is_dir());
        assert_eq!(log_dir.path(), sub);
    }

    #[test]
    fn discover_existing_segment() {
        let dir = tempfile::tempdir().unwrap();
        let segment_path = dir.path().join("segment_00000000000000000001.log");
        std::fs::write(&segment_path, b"").unwrap();
        let log_dir = LogDir::open(dir.path()).unwrap();
        let segs = log_dir.segments();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].base_offset, 1);
        assert_eq!(segs[0].log_path, segment_path);
    }

    #[test]
    fn lock_prevents_second_open() {
        let dir = tempfile::tempdir().unwrap();
        let first = LogDir::open(dir.path()).unwrap();
        let second = LogDir::open(dir.path());
        assert!(second.is_err());
        let err = second.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("lock"));
        drop(first);
        let third = LogDir::open(dir.path()).unwrap();
        drop(third);
    }
}
