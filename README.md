# durable-log

**Crash-safe, segmented commit log (WAL)** for Rust: append-only storage with checksums, index, and efficient iteration.

## What is it?

`durable-log` is an embeddable write-ahead log that provides:

- **Crash safety**: recovery by truncating partial/corrupt tail records on open.
- **Segmentation**: log files roll by size; segments are discovered and opened automatically.
- **Checksums**: per-record integrity verification.
- **Index**: fast offset→position lookup with automatic rebuild when missing or corrupt.
- **Concurrency**: single writer, multiple readers; scans can run while appending.

## Why use it?

Use it when you need a simple, reliable, pure-Rust log for event sourcing, durable queues, or replication state. No heavy runtime or external services—just add the crate and point it at a directory.

## Quick example

*(API will be expanded as the crate is built.)*

```rust
use durable_log::Result;

// Open or create a log directory
// let log = durable_log::Log::open("./data")?;
// log.append(b"hello")?;
```

## Guarantees

- **Single writer**: only one process should open the log for writing (enforced via lock file).
- **Durability**: configurable flush policy (e.g. fsync on append or manual).
- **Ordering**: offsets are monotonic; recovery preserves consistency up to the last valid record.

## Performance

Benchmarks and performance notes will be documented as the crate matures. See `cargo bench` and the *Performance* section in the docs.

## Documentation

- [Crate docs](https://docs.rs/durable-log) (when published)
- Design docs in `docs/`: file format, crash recovery, index layout.

## License

Dual-licensed under **MIT OR Apache-2.0**. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). We welcome issues and pull requests.
