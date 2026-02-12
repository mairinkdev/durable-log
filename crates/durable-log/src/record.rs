//! V1 on-disk record format: header encoding/decoding and frame layout.
//!
//! See the repository docs: `docs/file-format.md`.

use crate::error::Error;
use crate::Result;
use crc32fast::Hasher;
use std::io::{Cursor, Read, Write};

/// Magic number for durable-log segment files (ASCII "DLOG").
pub const MAGIC: u32 = 0x444C_4F47;

/// Current record format version.
pub const VERSION_V1: u8 = 1;

/// Record header size in bytes (fixed).
pub const HEADER_LEN: usize = 24;

/// Reserved flags for future use; must be 0 in v1.
pub const FLAGS_NONE: u8 = 0;

/// Fixed-size header for a single log record (v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordHeader {
    /// Must be [`MAGIC`].
    pub magic: u32,
    /// Format version; only [`VERSION_V1`] is supported.
    pub version: u8,
    /// Reserved; must be 0 in v1.
    pub flags: u8,
    /// Logical offset of this record (monotonic).
    pub offset: u64,
    /// Length of the payload in bytes.
    pub payload_len: u32,
    /// CRC-32 of the payload only (see docs).
    pub checksum: u32,
}

impl RecordHeader {
    /// Build a header for encoding. Checksum must be computed from the payload.
    #[must_use]
    pub const fn new(offset: u64, payload_len: u32, checksum: u32) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION_V1,
            flags: FLAGS_NONE,
            offset,
            payload_len,
            checksum,
        }
    }

    /// Compute CRC-32 of `payload` (used when encoding).
    #[must_use]
    pub fn checksum_of(payload: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(payload);
        hasher.finalize()
    }
}

/// Encodes a full record (header + payload) into a buffer. Uses little-endian.
///
/// Checksum is computed over the payload only.
///
/// # Errors
///
/// Returns an error if `payload.len()` exceeds `u32::MAX`.
///
/// # Panics
///
/// Never panics for valid input; writing to the internal `Vec` cannot fail.
pub fn encode_record(offset: u64, payload: &[u8]) -> Result<Vec<u8>> {
    let len = u32::try_from(payload.len()).map_err(|_| {
        Error::InvalidFormat(format!(
            "payload length {} exceeds maximum {}",
            payload.len(),
            u32::MAX
        ))
    })?;
    let checksum = RecordHeader::checksum_of(payload);
    let header = RecordHeader::new(offset, len, checksum);
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    encode_header_into(&header, &mut out).expect("write to Vec never fails");
    out.write_all(payload).expect("write to Vec never fails");
    Ok(out)
}

/// Encodes only the header into `out` (exactly [`HEADER_LEN`] bytes). Little-endian.
///
/// # Errors
///
/// Returns I/O errors from `out`.
pub fn encode_header_into(header: &RecordHeader, out: &mut impl Write) -> std::io::Result<()> {
    out.write_all(&header.magic.to_le_bytes())?;
    out.write_all(&[header.version, header.flags])?;
    out.write_all(&[0u8; 2])?; // reserved padding
    out.write_all(&header.offset.to_le_bytes())?;
    out.write_all(&header.payload_len.to_le_bytes())?;
    out.write_all(&header.checksum.to_le_bytes())?;
    Ok(())
}

/// Decodes a header from the first [`HEADER_LEN`] bytes. Fails if magic or version is invalid.
///
/// # Errors
///
/// Returns [`Error::InvalidFormat`] for wrong magic, unsupported version, or truncated input.
/// Returns I/O error only if the cursor read fails (e.g. truncated slice).
pub fn decode_header(bytes: &[u8]) -> Result<RecordHeader> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::InvalidFormat(format!(
            "header too short: {} bytes (need {})",
            bytes.len(),
            HEADER_LEN
        )));
    }
    let mut c = Cursor::new(bytes);
    let magic = read_u32_le(&mut c)?;
    if magic != MAGIC {
        return Err(Error::InvalidFormat(format!(
            "invalid magic: 0x{magic:08X} (expected 0x{MAGIC:08X})"
        )));
    }
    let mut ver_buf = [0u8; 1];
    c.read_exact(&mut ver_buf)?;
    let version = ver_buf[0];
    if version != VERSION_V1 {
        return Err(Error::InvalidFormat(format!(
            "unsupported version: {version} (expected {VERSION_V1})"
        )));
    }
    let mut flags_buf = [0u8; 1];
    c.read_exact(&mut flags_buf)?;
    let mut reserved = [0u8; 2];
    c.read_exact(&mut reserved)?;
    let offset = read_u64_le(&mut c)?;
    let payload_len = read_u32_le(&mut c)?;
    let checksum = read_u32_le(&mut c)?;
    Ok(RecordHeader {
        magic,
        version,
        flags: flags_buf[0],
        offset,
        payload_len,
        checksum,
    })
}

/// Decodes a full record (header + payload) from `bytes`. Validates magic and version only;
/// checksum validation is left to the caller (see Day 5).
///
/// # Errors
///
/// Returns [`Error::InvalidFormat`] for invalid header or truncated payload.
pub fn decode_record(bytes: &[u8]) -> Result<(RecordHeader, &[u8])> {
    let header = decode_header(bytes)?;
    let payload_start = HEADER_LEN;
    let end = payload_start
        .checked_add(header.payload_len as usize)
        .ok_or_else(|| {
            Error::InvalidFormat(format!(
                "payload length {} would overflow input (len {})",
                header.payload_len,
                bytes.len()
            ))
        })?;
    if bytes.len() < end {
        return Err(Error::InvalidFormat(format!(
            "record truncated: need {} bytes for payload, have {}",
            header.payload_len,
            bytes.len().saturating_sub(HEADER_LEN)
        )));
    }
    let payload = &bytes[payload_start..end];
    Ok((header, payload))
}

fn read_u32_le(r: &mut impl Read) -> std::io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64_le(r: &mut impl Read) -> std::io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = RecordHeader::new(42, 10, 0x1234_5678);
        let mut buf = Vec::new();
        encode_header_into(&h, &mut buf).unwrap();
        assert_eq!(buf.len(), HEADER_LEN);
        let decoded = decode_header(&buf).unwrap();
        assert_eq!(decoded.magic, MAGIC);
        assert_eq!(decoded.version, VERSION_V1);
        assert_eq!(decoded.offset, 42);
        assert_eq!(decoded.payload_len, 10);
        assert_eq!(decoded.checksum, 0x1234_5678);
    }

    #[test]
    fn record_roundtrip() {
        let payload = b"hello world";
        let encoded = encode_record(1, payload).unwrap();
        let (header, decoded_payload) = decode_record(&encoded).unwrap();
        assert_eq!(header.offset, 1);
        assert_eq!(header.payload_len, 11);
        assert_eq!(header.checksum, RecordHeader::checksum_of(payload));
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn invalid_magic_fails() {
        let mut buf = vec![0u8; HEADER_LEN];
        buf[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        buf[4] = VERSION_V1;
        let err = decode_header(&buf).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("invalid magic"), "{}", s);
    }

    #[test]
    fn invalid_version_fails() {
        let mut buf = vec![0u8; HEADER_LEN];
        buf[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        buf[4] = 99;
        let err = decode_header(&buf).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("unsupported version"), "{}", s);
    }

    #[test]
    fn header_too_short_fails() {
        let err = decode_header(&[0u8; 8]).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("too short"), "{}", s);
    }

    #[test]
    fn record_truncated_payload_fails() {
        let payload = b"hello";
        let mut encoded = encode_record(0, payload).unwrap();
        encoded.truncate(HEADER_LEN + 2); // truncate payload
        let err = decode_record(&encoded).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("truncated") || s.contains("overflow"), "{}", s);
    }

    /// Golden test: encoding a known record produces exact expected bytes (header part).
    #[test]
    fn golden_encode_header_bytes() {
        // offset=0, payload_len=3, checksum of "foo" (precomputed)
        let payload = b"foo";
        let expected_checksum = RecordHeader::checksum_of(payload);
        let encoded = encode_record(0, payload).unwrap();
        assert_eq!(encoded.len(), HEADER_LEN + 3);
        assert_eq!(&encoded[0..4], MAGIC.to_le_bytes());
        assert_eq!(encoded[4], VERSION_V1);
        assert_eq!(encoded[5], FLAGS_NONE);
        assert_eq!(&encoded[8..16], 0u64.to_le_bytes());
        assert_eq!(&encoded[16..20], 3u32.to_le_bytes());
        assert_eq!(&encoded[20..24], expected_checksum.to_le_bytes());
        assert_eq!(&encoded[24..], payload);
    }

    /// Golden test: decode then re-encode yields identical bytes.
    #[test]
    fn golden_decode_reencode_roundtrip() {
        let payload = b"golden roundtrip";
        let encoded = encode_record(100, payload).unwrap();
        let (header, decoded_payload) = decode_record(&encoded).unwrap();
        let reencoded = encode_record(header.offset, decoded_payload).unwrap();
        assert_eq!(
            encoded, reencoded,
            "decode then re-encode must match original"
        );
    }
}
