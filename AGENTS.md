# Repository instructions for coding agents

Read the relevant files in `docs/specs/` and `docs/adr/` before editing. Behaviour changes require a specification update; architectural changes require a new or superseding ADR. Keep changes narrow and reviewable.

## Non-negotiable boundaries

- `loremesh-core` must not depend on presentation, storage implementations, network clients, vendors, or databases.
- Keep Graphify, LLM, CI, and documentation vendors behind ports and subprocess/network adapters outside the domain.
- Sources and immutable snapshots are authoritative. Derived data must remain rebuildable.
- Never merge personal feedback into organization knowledge implicitly.
- Do not silently change public CLI or persisted/exported formats.
- Do not expose private workspace content, paths, credentials, or tokens in logs, fixtures, snapshots, or errors.
- Production code must not use `unwrap`, `expect`, `panic!`, `todo!`, or `unimplemented!` for recoverable conditions. Unsafe Rust is forbidden.
- Tests must be deterministic, order-independent, offline, and contained in temporary directories. Do not weaken lints or tests merely to make CI pass, replace meaningful assertions with snapshots only, or add network-dependent tests.

## Required workflow

Add unit or integration coverage for every behavioural change and a regression test for every defect fix. Before finishing, run:

```console
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo deny check
./scripts/check-architecture.sh
cargo test -p loremesh-storage --test corpus_lifecycle
cargo run -p loremesh-public-corpus -- verify-profile
```

If installed, also run `cargo machete` and `cargo llvm-cov --workspace --all-features --html`. Summarize decisions, changed files, exact commands and results, and remaining risks. Leave the working tree clean.
