# Components

| Component | Owns | Must not own |
|---|---|---|
| `loremesh-core` | identifiers, artifacts, evidence, findings, feedback, traces, ports, invariants | I/O implementations, UI, SQL, vendor types |
| `loremesh-storage` | workspace layout, safe import, object files, SQLite schema and repository | business presentation, remote access |
| `loremesh-report` | report projection and JSON/CSV/Markdown/HTML rendering | terminal state, persistence |
| `loremesh-tui` | view models, state transitions, Ratatui rendering, pure grid/chart/code/Markdown presentation models | domain mutation, filesystem access, or process execution |
| `loremesh` | CLI, use-case orchestration, dependency construction, workspace-safe file access, explicitly enabled local process execution, error context | reusable domain rules |

Only boundaries exercised today receive ports. The storage repository is synchronous because foundation operations are local and small; async orchestration is deferred until concurrent connectors justify Tokio.
