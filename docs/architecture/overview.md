# Architecture overview

LoreMesh is a Rust modular monolith using ports and adapters.

```text
CLI / TUI -> application composition -> core ports and domain
                                  |-> filesystem + SQLite adapter
                                  |-> report renderers
future external engines <-> bounded subprocess adapter <-> core port
```

`loremesh-core` owns invariants and abstractions. `loremesh-storage` implements local persistence. `loremesh-report` transforms domain state into renderer-independent reports and serializes them. `loremesh-tui` maps state into testable view models and Ratatui widgets. The `loremesh` binary parses commands and wires concrete implementations.

Sources and immutable snapshots are authoritative. SQLite, traces, findings, tables, and rendered files are derived or user-authored metadata and are replaceable. Constructors make dependencies explicit; there is no global service locator.
