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

Inside the TUI, press `/` to enter a command, `Tab` or `Shift-Tab` to move focus,
and `q`, `Esc`, `/quit`, or `/exit` to leave. Use `/help` for the command list.
For example, `/trace` opens lineage and `/save current --format markdown-mermaid
--output trace.md` saves the active structured view without overwriting an existing
file.

Workbench data and source commands are also available:

```text
/table load results.csv
/table filter status failed
/table sort duration desc
/chart hbar name duration
/browse src
/open README.md
/search lineage
/shell status
```

Every completed command returns focus to the upper investigation timeline. `/help`
opens the full multiline command reference there. Use `/demo table`, `/demo chart`,
`/demo markdown`, `/demo code`, or `/demo shell` to preview capabilities without
creating input files or executing a command.

Local shell execution is disabled on every startup. `/shell enable` enables it only
for the current TUI session; commands have the user's operating-system permissions
and may access files or networks. Output is bounded, marked untrusted, and is not
automatically saved or treated as evidence.

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
