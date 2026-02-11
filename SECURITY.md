# Security

## Reporting a vulnerability

If you believe you have found a security vulnerability in durable-log, please report it responsibly:

- **Do not** open a public issue.
- Contact the maintainers privately (e.g. via the repository’s security contact or maintainer email, if published).
- Provide a clear description of the issue, steps to reproduce, and impact.

We will acknowledge your report and work with you on a fix and disclosure timeline.

## Security considerations

- durable-log is a local storage library (WAL/commit log). It does not implement network or cryptographic protocols.
- Data is stored on disk with per-record checksums for corruption detection, not for confidentiality.
- For sensitive data, consider encryption at the application layer before appending to the log.
