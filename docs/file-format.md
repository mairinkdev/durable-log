# durable-log file format (v1)

This document describes the on-disk layout of durable-log segment records. It is the single source of truth for the v1 format.

## Endianness and alignment

- All multi-byte integer fields are **little-endian**.
- The record header is **fixed size** (24 bytes). There is no alignment requirement beyond the header; the payload immediately follows.

## Record layout

A **record** consists of a **header** (24 bytes) followed by **payload** (variable length).

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          magic (u32)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| version (u8)  |  flags (u8)   |      reserved (u16)           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        offset (u64)                           |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   payload_len (u32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     checksum (u32)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      payload (variable)                       |
...
```

### Header fields (24 bytes total)

| Offset | Size | Field        | Description |
|--------|------|--------------|-------------|
| 0      | 4    | magic        | Must be `0x444C4F47` (ASCII "DLOG"). Used to detect non–durable-log files. |
| 4      | 1    | version      | Format version. Only `1` is defined. |
| 5      | 1    | flags        | Reserved; must be `0` in v1. |
| 6      | 2    | reserved     | Padding; must be `0`. |
| 8      | 8    | offset       | Logical offset of this record (monotonic per log). |
| 16     | 4    | payload_len  | Length of the payload in bytes. |
| 20     | 4    | checksum     | CRC-32 of the **payload only** (see below). |

### Payload

- Length is given by `payload_len`. There is no trailing delimiter; the next record (if any) starts at byte `24 + payload_len` of the current record.
- **Checksum scope**: the `checksum` field is the CRC-32 (IEEE polynomial, same as `crc32fast`) of the raw payload bytes only. The header is not included in the checksum.

## Versioning

- **Version 1**: format described above.
- Readers must reject unknown `version` values (e.g. return an error or skip). New versions may add optional trailing fields or new record types in the future; v1 will remain decodable.

## Segment files

- Segment data files use the extension `.log` and contain a sequence of records with no extra framing between records.
- Offsets are assigned monotonically; the first record in a segment may have any `offset` (the segment’s base offset). Segment naming and index layout are described in other docs (`index.md`, etc.).
