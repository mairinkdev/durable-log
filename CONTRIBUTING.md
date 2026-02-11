# Contributing to durable-log

Thank you for your interest in contributing.

## Development setup

- Rust 1.75 or later (see `rust-toolchain.toml`).
- Run `cargo test`, `cargo fmt --all -- --check`, and `cargo clippy --all-targets -- -D warnings` before submitting.

## Code standards

- **Pure Rust**: avoid heavy dependencies; prefer small, mature crates.
- **Safety**: correctness and safety first. If you use `unsafe`, isolate it in a dedicated module and document invariants.
- **Errors**: use `Result<T, E>`; avoid `unwrap()` on production paths.
- **API**: keep the public API minimal and clear; internals stay in modules.

## Pull requests

1. Open an issue or comment on an existing one to align on the change.
2. Branch from `main` (or `master`), make small, reviewable commits.
3. Ensure CI passes (format, clippy, tests, docs, cargo-deny, cargo-audit).
4. Update documentation and tests as needed.

## License

By contributing, you agree that your contributions will be licensed under the same dual license as the project: **MIT OR Apache-2.0**.
