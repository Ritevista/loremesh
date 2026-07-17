# ADR 0002: Rust workspace and dependency baseline

- Status: Accepted
- Date: 2026-07-17

## Context
The foundation needs strong types, portable binaries, deterministic tooling, CLI parsing, local persistence, terminal display, and safe serialization.

## Decision
Follow the latest stable Rust channel and record 1.97.1 as the initial MSRV resolved when this decision was accepted. Raising the MSRV requires release notes and CI review. Use a workspace with core, storage, report, TUI, and binary crates. Use Serde for canonical structured data, Thiserror for library errors, SHA-2/hex for content IDs, Clap for CLI, Anyhow only in the binary, Rusqlite with bundled SQLite, Ratatui/Crossterm for the terminal, CSV for tables, and Tracing for metadata-only diagnostics. Tests use Tempfile, Assert Command, Predicates, and Proptest. No Tokio is added until concurrency exists; no HTML template engine is needed for the small escaped renderer. Child-process deadlines use the standard library's portable `try_wait` polling in the composition root, avoiding a signal-handler dependency and async runtime. Interactive local shells use `portable-pty` for the narrow cross-platform PTY boundary; it remains outside domain, storage, report, and presentation logic.

## Consequences
Dependencies are mature and focused, but bundled SQLite increases build time and binary size, while `portable-pty` adds platform-specific transitive crates that must be exercised in the operating-system CI matrix. Serde formats remain explicitly pre-stable. Ratatui stays outside core. Workspace dependency centralization and `cargo-deny` constrain duplicates, sources, and licenses. Maintainers review advisories and unused dependencies in CI.
