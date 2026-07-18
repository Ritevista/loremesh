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

The committed heterogeneous corpus can be imported and searched entirely offline:

```console
just corpus-fixture
cd target/test-corpora/fixture-workspace
../../debug/loremesh index search "bounded retry"
```

`just corpus-public-verify` validates the pinned Kubernetes profile without network access. `just corpus-public` is the explicit network-enabled build and writes only below `target/test-corpora/`. Scale recipes `just corpus-scale-100m`, `corpus-scale-500m`, `corpus-scale-1g`, and `corpus-scale-2g` require the underlying large-output acknowledgement and never run in CI.

Import a generated scale corpus from an initialized workspace with the matching explicit opt-in:

```console
loremesh corpus open ../scale-100m
```

This discovers the manifest, applies bounded local limits, imports the corpus, builds the disposable knowledge index, and opens the TUI. Use `--no-tui` for automation. Large import mode remains bounded and performs no network access or imported-code execution.

Inside the TUI, press `/` to enter a command, `Tab` or `Shift-Tab` to move focus,
Page Up/Down or Home/End to scroll results, and `q`, `/quit`, or `/exit` to leave.
`Esc` safely returns to the timeline and never exits the application. Use `/help` for the command list.
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
/shell
```

Every completed command returns focus to the upper investigation timeline. `/help`
opens the full multiline command reference there. Use `/demo table`, `/demo chart`,
`/demo markdown`, `/demo code`, or `/demo shell` to preview capabilities without
creating input files or executing a command.

`/demo chart` shows the responsive multi-series chart renderer. Structured tables
and charts use the full result width, semantic focus/status colors, stable series
colors, labels and values that remain meaningful without color, and a compact
text fallback for narrow terminals.

`/shell` starts a persistent local shell in the workspace and keeps the bottom
composer focused. Type commands normally; their output streams into the scrollable
investigation timeline. Use Ctrl-C to interrupt, and `/exit` or Ctrl-D to return to
LoreMesh command mode. `/quit` exits the application. The shell has the user's
operating-system permissions and may access files or networks. Scrollback is bounded,
is not automatically saved, and is never treated as evidence.

## Principles

- Imported content stays local unless a user explicitly configures a future network adapter.
- Source snapshots are authoritative; artifacts, evidence, findings, accepted relationships, traces, and feedback have explicit canonical lifecycles. Indexes and external-engine candidates are replaceable derivatives.
- Findings carry evidence and separate source lineage from processing lineage.
- Personal feedback is isolated from organization knowledge.
- The core is vendor-, UI-, database-, and network-independent.

Start with the [product vision](docs/product/vision.md), [architecture overview](docs/architecture/overview.md), [code and rendering review map](docs/architecture/code-structure-and-rendering.md), and [specifications](docs/specs/README.md). Contributors should read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

## Validation

```console
just check
just test
just ci
```

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
