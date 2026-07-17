# ADR 0008: Make local tool execution an explicit trust boundary

- Status: Accepted
- Date: 2026-07-17

## Context

Workbench users need local command-line tools for transformations and investigations. A shell can read private files, inherit credentials, access networks, modify the workspace, and produce untrusted output. Treating it like an ordinary slash command would violate the no-hidden-network and content-safety principles.

## Decision

Keep process execution in the application composition root behind a `LocalToolRunner` port. It is disabled at startup and requires an explicit per-session enable action. Commands run visibly in the workspace directory with a deadline and bounded captured output. LoreMesh does not silently persist command text or output, interpret output as evidence, or grant it canonical status. Shell metacharacter interpretation is available only through the explicitly named shell command; future direct-program execution should remain a safer separate mode.

The terminal shell owns interaction and cancellation state, while the runner owns process lifecycle and resource limits. No execution API belongs in `loremesh-core`.

## Consequences

Users can compose familiar local tools, including pipelines, while accepting the operating-system permissions of their own shell. Enabling is deliberately frictionful and session-local. Strong sandboxing is deferred because portable process sandboxing requires platform-specific design; the UI must not imply that workspace-scoped working directory is a security sandbox.
