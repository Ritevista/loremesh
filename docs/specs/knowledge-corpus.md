# Knowledge corpus

## Status

Accepted for the corpus foundation.

## Problem

LoreMesh needs to import heterogeneous, versioned engineering knowledge without making a search index, graph engine, vendor export, or mutable source file authoritative.

## Goals

Import a bounded manifest of Markdown, images, issue records, code references, and explicit relationships; preserve immutable snapshots; diagnose corpus health; and keep findings attached to the snapshot originally cited.

## Non-goals

Remote synchronization, enterprise connectors, executing imported code, LLMs, embeddings, RAG, semantic search, Graphify execution, Tree-sitter, or automatic relationship inference.

## User scenarios

An engineer imports a repository-owned fixture, reimports it without duplicate snapshots, changes one source and receives a new snapshot, searches its documents, and sees broken or stale references without losing historical evidence.

## Functional requirements

`loremesh corpus import <manifest>` validates the whole manifest before importing entries. Content paths resolve beneath the manifest directory, symlinks and traversal are rejected, text is valid UTF-8, and configured byte/count limits are enforced. The default accepts manifests up to 2 MiB, 10,000 artifacts, 16 MiB per artifact, and 512 MiB total content. `--allow-large` is an explicit local-only opt-in bounded at a 256 MiB manifest, 100,000 artifacts, 64 MiB per artifact, and 3 GiB total content so the documented 2 GB scale profile can be imported without silently removing resource controls. Each logical source has immutable content-addressed snapshots; unchanged bytes reuse a snapshot and changed bytes create another. Current-source state is separate from historical references. Import produces a structured report with discovered/imported/unchanged counts and diagnostics for missing files, broken references, duplicate logical identities, invalid metadata, and relationship failures.

## Domain model

Canonical records are `Source`, `SourceSnapshot`, `Artifact`, `ArtifactReference`, `EvidenceReference`, `CodeReference`, `Relationship`, `Finding`, `Trace`, and `Feedback`. A manifest is a portable import description, not the canonical database. Index documents and external relationship candidates are derived inputs.

## Interfaces

The storage adapter accepts a validated manifest plus explicit resource limits and returns `CorpusImportResult`. The CLI renders its diagnostics through `loremesh-report`. Repository queries expose snapshots by source and current snapshot identity without redirecting historical artifact IDs.

## Invariants

Source location identifies one logical source; `(source, digest)` identifies one immutable snapshot; artifacts never change snapshot; current snapshot may advance; old evidence remains resolvable; imports never execute content; deleting derived data cannot delete canonical objects, metadata, findings, relationships, or feedback.

## Failure modes

Invalid schema, duplicate identity, traversal, symlink, missing content, oversized input, invalid UTF-8, digest mismatch, malformed relationship, resource-limit exhaustion, database failure, and partial object writes are reported without claiming complete success.

## Security and privacy implications

All content is untrusted and local. Imports perform no network access, do not execute files, do not log bodies or absolute paths, and reject paths outside the corpus root. Shared reports use logical locations. Public downloads are a separate explicit tool.

## Observability requirements

Diagnostics may include logical path, stable ID, category, count, and outcome. They must omit full document bodies, host paths, credentials, and private command output.

## Acceptance criteria

The tiny fixture imports offline; repeat import is idempotent; changed bytes create a snapshot; old evidence remains attached to its old artifact; malformed/traversal inputs fail safely; duplicate and broken references are reported; and a report contains deterministic counts.

## Test strategy

Use core unit tests for identities and relationships, storage tests with temporary copies for lifecycle behavior, fixture integration tests for health diagnostics, and CLI tests for output and exit behavior. Tests never access the network.

## Deferred decisions

Streaming multi-gigabyte import, image metadata extraction, attachment previews, incremental directory watching, deletion/tombstones, locking, migrations across stable releases, and stale-evidence TUI interaction.
