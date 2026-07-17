# LoreMesh

LoreMesh is a local-first engineering knowledge and investigation workbench. It imports immutable source snapshots, records evidence-backed findings, explains lineage, and renders reusable reports without requiring a network, graph engine, embeddings, or an LLM.

> **Status:** foundation prototype. File formats and APIs may change before the first stable release.

## Foundation demo

Prerequisites: the latest stable Rust toolchain (currently 1.97.1), Cargo, and `just` (optional). `rust-toolchain.toml` keeps contributors on the stable channel; the manifest records 1.97.1 as the initial minimum supported version.

```console
cargo run -p loremesh -- workspace init /tmp/loremesh-demo
cd /tmp/loremesh-demo
loremesh demo seed
loremesh workspace status
loremesh report export --format html --output report.html
loremesh tui
```

When running through Cargo, replace `loremesh` with `cargo run --manifest-path /path/to/loremesh/Cargo.toml -p loremesh --`. `just demo` creates a deterministic workspace under `target/demo-workspace` and prints its status.

## Principles

- Imported content stays local unless a user explicitly configures a future network adapter.
- Source snapshots are authoritative; findings, graphs, indexes, and reports are replaceable derivatives.
- Findings carry evidence and separate source lineage from processing lineage.
- Personal feedback is isolated from organization knowledge.
- The core is vendor-, UI-, database-, and network-independent.

Start with the [product vision](docs/product/vision.md), [architecture overview](docs/architecture/overview.md), and [specifications](docs/specs/README.md). Contributors should read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

## Validation

```console
just check
just test
just ci
```

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
