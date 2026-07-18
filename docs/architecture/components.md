# Components

| Component | Owns | Must not own |
|---|---|---|
| `loremesh-core` | identifiers, artifacts, evidence, findings, relationships, feedback, traces, lexical-index ports, invariants | I/O implementations, UI, SQL, search-engine or vendor types |
| `loremesh-storage` | workspace layout, safe corpus import, object files, SQLite schema/repository, Tantivy adapter | business presentation, remote access, external-engine policy |
| `loremesh-report` | report projection and JSON/CSV/Markdown/HTML rendering | terminal state, persistence |
| `loremesh-tui` | view models, state transitions, semantic theme, responsive Ratatui table/chart rendering, interactive grid state, renderer-neutral chart data, code/Markdown presentation models | domain mutation, filesystem access, process execution, canonical report formats |
| `loremesh` | CLI, use-case orchestration, dependency construction, conversions between view/report models, workspace-safe file access, explicitly requested local process execution, error context | reusable domain rules or reusable renderer logic |

Only boundaries exercised today receive ports. The storage repository is synchronous because foundation operations are local and small; async orchestration is deferred until concurrent connectors justify Tokio.

The three rectangular table types are intentionally distinct: `TableModel` is a canonical export/report block, `DataGrid` is mutable interactive query state, and `ViewTable` is a disposable terminal projection. They must be converted explicitly rather than shared across those responsibilities. See [Code structure and rendering boundaries](code-structure-and-rendering.md).
