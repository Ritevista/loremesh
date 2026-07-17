# Workspace foundation

## Status
Accepted for the foundation.

## Problem
An engineer needs a predictable local container for authoritative snapshots and derived metadata without a service.

## Goals
Initialize, discover, validate, and summarize an offline workspace; preserve deterministic layout and actionable failures.

## Non-goals
Workspace synchronization, encryption, multi-user locking, migrations across stable releases, and remote sources.

## User scenarios
An engineer initializes a path, runs commands from its root, imports a file, seeds the demo, checks status, and diagnoses configuration with `doctor`.

## Functional requirements
`workspace init <path>` creates `<path>/.loremesh/objects` and a SQLite database atomically enough that rerunning is idempotent. Commands other than init discover `.loremesh` from the current directory. Status reports counts without source content. Initialization must reject a file path and incompatible existing state. All operations work offline.

## Domain model
`Workspace` has a typed ID, display name, and root. `Source` names an origin using a workspace-relative logical location. `SourceSnapshot` binds source, SHA-256 digest, and byte length.

## Interfaces
CLI: `loremesh workspace init <path>`, `loremesh workspace status`, `loremesh doctor`. Storage exposes initialize/open/status through a repository used by the binary.

## Invariants
Workspace metadata stays below `.loremesh`; object filenames are lowercase SHA-256; absolute source paths are not persisted; init never deletes existing user content.

## Failure modes
Permission, invalid path, incompatible schema, corrupt database, and I/O failures produce non-zero exits and contextual stderr without content.

## Security and privacy implications
No network or telemetry. Paths in shared output are logical paths. Symlink and size handling is conservative.

## Observability requirements
CLI output names the operation and workspace, but diagnostics never include imported bytes or secrets. Future structured logs are opt-in.

## Acceptance criteria
Init creates a valid workspace; rerun succeeds; status on an uninitialized directory fails; status counts seeded data; all tests use temporary directories.

## Test strategy
Unit-test layout validation; integration-test initialization/idempotency/discovery; CLI-test exit codes and messages; contract-test repository status.

## Deferred decisions
Locking, schema migration guarantees, encryption, configurable metadata location, garbage collection, and workspace bundles.
