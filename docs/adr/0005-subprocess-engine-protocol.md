# ADR 0005: External analysis engines use subprocess protocols

- Status: Proposed
- Date: 2026-07-17

## Context
Graphify may be the first optional graph engine, but the core must work without it and must not embed its runtime or models.

## Decision
When implemented, invoke external engines as explicitly configured executables using a versioned newline-delimited JSON protocol. Do not invoke a shell. Bound input/output, sanitize environment, impose timeout/cancellation, capture metadata-only diagnostics, and validate every response into canonical core models. Capability negotiation and protocol fixtures form contract tests.

## Consequences
Engines can use independent languages and failure isolation is clearer. Process startup and protocol evolution add complexity. This ADR remains proposed: no engine port or dependency is created until the next vertical slice defines concrete operations and threat limits.
