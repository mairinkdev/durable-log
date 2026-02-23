//! Core log management: append, segments, and index.

use crate::error::Error;
use crate::log_dir::LogDir;
use crate::record::{
    decode_header, encode_record, HEADER_LEN, INDEX_ENTRY_LEN,
};
use crate::segment::{SegmentId, SegmentInfo};
use crate::Result;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path};

/// Configuration for the log.
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum size of a segment file in bytes before rolling to a new one.
    pub max_segment_bytes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_segment_bytes: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// A crash-safe, segmented log.
#[derive(Debug)]
pub struct Log {
    dir: LogDir,
    config: Config,
    active_segment: ActiveSegment,
}

#[derive(Debug)]
struct ActiveSegment {
    info: SegmentInfo,
    log_file: File,
    idx_file: File,
    current_size: u64,
    next_offset: u64,
}

impl Log {
    /// Opens the log in the given directory. Creates it if missing.
    /// Performs recovery if the last segment is corrupted.
    pub fn open(path: impl AsRef<Path>, config: Config) -> Result<Self> {
        let dir = LogDir::open(path)?;
        let segments = dir.segments();

        let active_segment = if let Some(last_info) = segments.last() {
            Self::open_active_segment(last_info.clone())?
        } else {
            Self::create_segment(&dir, 0)?
        };

        let mut log = Self {
            dir,
            config,
            active_segment,
        };

        log.recover()?;
        Ok(log)
    }

    fn open_active_segment(info: SegmentInfo) -> Result<ActiveSegment> {
        let log_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&info.log_path)?;
        
        let idx_path = info.log_path.with_extension("idx");
        let idx_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&idx_path)?;

        let current_size = log_file.metadata()?.len();
        
        // We will determine next_offset during recovery.
        // For now, placeholder.
        Ok(ActiveSegment {
            info,
            log_file,
            idx_file,
            current_size,
            next_offset: 0, 
        })
    }

    fn create_segment(dir: &LogDir, base_offset: u64) -> Result<ActiveSegment> {
        let id = SegmentId(base_offset);
        let log_path = dir.path().join(id.log_filename());
        let idx_path = log_path.with_extension("idx");

        let log_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&log_path)?;

        let idx_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&idx_path)?;

        Ok(ActiveSegment {
            info: SegmentInfo {
                base_offset,
                log_path,
            },
            log_file,
            idx_file,
            current_size: 0,
            next_offset: base_offset,
        })
    }

    /// Appends a payload to the log.
    pub fn append(&mut self, payload: &[u8]) -> Result<u64> {
        let encoded = encode_record(self.active_segment.next_offset, payload)?;
        let record_len = encoded.len() as u64;

        if self.active_segment.current_size + record_len > self.config.max_segment_bytes {
            self.roll()?;
        }

        let offset = self.active_segment.next_offset;
        let pos = self.active_segment.current_size;

        // Write record to .log
        self.active_segment.log_file.seek(SeekFrom::End(0))?;
        self.active_segment.log_file.write_all(&encoded)?;
        
        // Write index entry to .idx
        self.active_segment.idx_file.seek(SeekFrom::End(0))?;
        self.write_index_entry(offset, pos)?;

        self.active_segment.current_size += record_len;
        self.active_segment.next_offset += 1;

        Ok(offset)
    }

    fn write_index_entry(&mut self, offset: u64, pos: u64) -> Result<()> {
        self.active_segment.idx_file.write_all(&offset.to_le_bytes())?;
        self.active_segment.idx_file.write_all(&pos.to_le_bytes())?;
        Ok(())
    }

    fn roll(&mut self) -> Result<()> {
        let next_offset = self.active_segment.next_offset;
        self.active_segment = Self::create_segment(&self.dir, next_offset)?;
        Ok(())
    }

    /// Flushes all pending writes to disk.
    pub fn flush(&mut self) -> Result<()> {
        self.active_segment.log_file.sync_all()?;
        self.active_segment.idx_file.sync_all()?;
        Ok(())
    }

    /// Scans the last segment to find the last valid record and truncate corruption.
    fn recover(&mut self) -> Result<()> {
        let mut file = &self.active_segment.log_file;
        file.seek(SeekFrom::Start(0))?;
        
        let mut last_valid_pos = 0;
        let mut next_offset = self.active_segment.info.base_offset;
        let mut buf = [0u8; HEADER_LEN];

        loop {
            match file.read_exact(&mut buf) {
                Ok(_) => {
                    let header = match decode_header(&buf) {
                        Ok(h) => h,
                        Err(_) => break, // Likely partial record or EOF
                    };
                    
                    if header.offset != next_offset {
                        // Offset mismatch, possible corruption
                        break;
                    }

                    // Seek past payload
                    if let Err(_) = file.seek(SeekFrom::Current(header.payload_len as i64)) {
                        break;
                    }

                    last_valid_pos = file.stream_position()?;
                    next_offset += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
        }

        if last_valid_pos < self.active_segment.current_size {
            // Truncate corrupted tail
            self.active_segment.log_file.set_len(last_valid_pos)?;
            self.active_segment.current_size = last_valid_pos;
            
            // Also truncate index to match
            let idx_len = (next_offset - self.active_segment.info.base_offset) * INDEX_ENTRY_LEN as u64;
            self.active_segment.idx_file.set_len(idx_len)?;
        }

        self.active_segment.next_offset = next_offset;
        self.active_segment.log_file.seek(SeekFrom::End(0))?;
        self.active_segment.idx_file.seek(SeekFrom::End(0))?;
        
        Ok(())
    }

    /// Reads a record at the given offset.
    pub fn read(&mut self, offset: u64) -> Result<Vec<u8>> {
        // Simple implementation: scan segments to find the one containing offset.
        // Then use index if it's the active segment, or linear scan for now.
        // Real implementation should use index for all segments.
        
        // For now, let's assume it's in the active segment for Day 7 basic test.
        if offset < self.active_segment.info.base_offset {
             return Err(Error::InvalidFormat(format!("Offset {} is before base offset {}", offset, self.active_segment.info.base_offset)));
        }

        let idx_pos = (offset - self.active_segment.info.base_offset) * INDEX_ENTRY_LEN as u64;
        if idx_pos + INDEX_ENTRY_LEN as u64 > self.active_segment.idx_file.metadata()?.len() {
             return Err(Error::InvalidFormat(format!("Offset {} not found in index", offset)));
        }

        self.active_segment.idx_file.seek(SeekFrom::Start(idx_pos))?;
        let mut entry_buf = [0u8; INDEX_ENTRY_LEN];
        self.active_segment.idx_file.read_exact(&mut entry_buf)?;
        
        let mut cursor = std::io::Cursor::new(&entry_buf);
        let mut b8 = [0u8; 8];
        cursor.read_exact(&mut b8)?;
        let entry_offset = u64::from_le_bytes(b8);
        cursor.read_exact(&mut b8)?;
        let entry_pos = u64::from_le_bytes(b8);

        if entry_offset != offset {
            return Err(Error::Corruption(format!("Index entry offset mismatch: expected {}, got {}", offset, entry_offset)));
        }

        self.active_segment.log_file.seek(SeekFrom::Start(entry_pos))?;
        let mut header_buf = [0u8; HEADER_LEN];
        self.active_segment.log_file.read_exact(&mut header_buf)?;
        let header = decode_header(&header_buf)?;
        
        let mut payload = vec![0u8; header.payload_len as usize];
        self.active_segment.log_file.read_exact(&mut payload)?;
        
        header.validate_checksum(&payload)?;
        
        Ok(payload)
    }
}

#[cfg(test)]
mod log_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_append_and_read() {
        let dir = tempdir().unwrap();
        let mut log = Log::open(dir.path(), Config::default()).unwrap();
        
        let offset0 = log.append(b"first").unwrap();
        let offset1 = log.append(b"second").unwrap();
        
        assert_eq!(offset0, 0);
        assert_eq!(offset1, 1);
        
        assert_eq!(log.read(0).unwrap(), b"first");
        assert_eq!(log.read(1).unwrap(), b"second");
    }

    #[test]
    fn test_segment_rolling() {
        let dir = tempdir().unwrap();
        // Tiny max_segment_bytes to force rolling
        let config = Config { max_segment_bytes: 30 }; 
        let path = dir.path().to_path_buf();

        {
            let mut log = Log::open(&path, config).unwrap();
            log.append(b"first").unwrap(); 
            log.append(b"second").unwrap(); 
        }
        
        let segments = crate::discover_segments(&path).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].base_offset, 0);
        assert_eq!(segments[1].base_offset, 1);

        // Reopen and check data
        let mut log = Log::open(&path, Config::default()).unwrap();
        assert_eq!(log.read(0).unwrap(), b"first");
        assert_eq!(log.read(1).unwrap(), b"second");
    }

    #[test]
    fn test_recovery_from_incomplete_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let log_path;
        
        {
            let mut log = Log::open(&path, Config::default()).expect("Failed to open log first time");
            log.append(b"valid").expect("Failed to append valid");
            log.flush().expect("Failed to flush");
            log_path = log.active_segment.info.log_path.clone();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Manually append a partial record header
        let mut f = None;
        for i in 0..50 {
            match OpenOptions::new().write(true).open(&log_path) {
                Ok(file) => {
                    f = Some(file);
                    break;
                }
                Err(_) if i < 49 => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("Failed to open manually: {:?}", e),
            }
        }
        let mut f = f.unwrap();
        f.seek(SeekFrom::End(0)).unwrap();
        f.write_all(&[0x44, 0x4C, 0x4F, 0x47]).unwrap(); // Magic only
        drop(f);
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Reopen should truncate the partial write
        // On Windows, we might need a retry because the OS takes time to release handles
        let mut log = None;
        for i in 0..50 {
            match Log::open(&path, Config::default()) {
                Ok(l) => {
                    log = Some(l);
                    break;
                }
                Err(_) if i < 49 => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("Failed to open for recovery: {:?}", e),
            }
        }
        let mut log = log.unwrap();
        assert_eq!(log.active_segment.next_offset, 1);
        assert_eq!(log.read(0).unwrap(), b"valid");
        
        // Should be able to append normally now
        log.append(b"new").unwrap();
        assert_eq!(log.read(1).unwrap(), b"new");
    }

    #[test]
    fn test_corruption_detection() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let log_file_path;

        {
            let mut log = Log::open(&path, Config::default()).unwrap();
            log.append(b"corrupt me").unwrap();
            log_file_path = log.active_segment.info.log_path.clone();
            log.flush().unwrap();
        }

        // Flip a bit in the payload (offset 24 + something)
        let mut data = std::fs::read(&log_file_path).unwrap();
        data[25] ^= 0xFF; 
        std::fs::write(&log_file_path, data).unwrap();

        let mut log = Log::open(&path, Config::default()).unwrap();
        let err = log.read(0).unwrap_err();
        assert!(err.to_string().contains("corruption") || err.to_string().contains("checksum"));
    }
}
