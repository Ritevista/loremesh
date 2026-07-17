# Trace model

## Status
Accepted for the foundation.

## Problem
A citation alone does not show the path from a finding through evidence to a snapshot or distinguish processing history.

## Goals
Represent a small validated directed trace with explicit origin and verification status, and return an evidence path.

## Non-goals
General graph queries, graph databases, automatic inference, confidence scores, or cross-workspace federation.

## User scenarios
An engineer selects a finding and sees finding → evidence → snapshot, including whether each edge is manual or deterministic and reviewed.

## Functional requirements
A trace adds uniquely identified typed nodes and edges whose endpoints exist. Duplicate nodes/edges, self-edges, and cycle-producing edges fail. `path(from,to)` returns a deterministic shortest path or a not-found result. A finding trace must connect each claim's evidence to its snapshot.

## Domain model
`Trace` contains `TraceNode`, `TraceEdge`, and validated `TracePath`. Nodes are finding, evidence, snapshot, or processing step. Edges carry `EdgeOrigin` and `VerificationStatus`.

## Interfaces
Core constructors/mutators return typed invariant errors. TUI and reports consume read-only node/edge/path projections.

## Invariants
Directed acyclic graph; referenced endpoints exist; path edges are contiguous; source and processing lineage labels remain explicit.

## Failure modes
Missing endpoint, duplicate ID, self-edge, cycle, disconnected path, and malformed persisted data are surfaced, never silently repaired.

## Security and privacy implications
Trace labels must not copy private source content. Scope checks precede future cross-scope trace sharing.

## Observability requirements
Diagnostics use trace/node IDs and invariant names, not evidence excerpts.

## Acceptance criteria
Demo trace yields the expected three-node path; cycle insertion fails; path selection is stable; the TUI lineage panel renders origin and verification.

## Test strategy
Unit/property tests for DAG/path invariants and deterministic traversal; integration test reconstructs the seeded trace from storage.

## Deferred decisions
Large-graph indexing, multiple paths, confidence, temporal validity, external engine contracts, and persistence versioning.
