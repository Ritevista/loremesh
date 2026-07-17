# Artifact and evidence model

## Status
Accepted for the foundation.

## Problem
Findings require stable citations even after an original local file changes.

## Goals
Import Markdown into immutable content-addressed snapshots and reference validated byte ranges as evidence.

## Non-goals
Live watching, image parsing, directory recursion, remote import, semantic chunks, or content deduplication policy beyond identical bytes.

## User scenarios
An engineer imports Markdown, sees stable metadata, and creates a claim citing an exact UTF-8 range in the snapshot.

## Functional requirements
`artifact import <file>` accepts a regular UTF-8 Markdown file within the size limit, stores its bytes by digest, and records source/snapshot/artifact metadata. Reimporting identical logical content is idempotent. Evidence must use a non-empty in-bounds byte range at UTF-8 boundaries and preserve an optional concise label.

## Domain model
`Source` → `SourceSnapshot` → `Artifact`; `ArtifactReference` identifies an artifact; `EvidenceReference` adds half-open byte offsets. IDs are typed and stable from canonical inputs.

## Interfaces
CLI import prints artifact and snapshot IDs. The repository accepts a file path and returns an import result; core constructors validate evidence.

## Invariants
Snapshots never mutate; digest matches stored bytes; artifact references cannot point to a different typed entity; evidence range satisfies `start < end <= byte_length` and UTF-8 boundaries when checked against content.

## Failure modes
Missing/non-file/symlink input, oversized bytes, invalid UTF-8, unsupported extension, traversal-like logical names, digest collision, storage failure, and invalid evidence fail without partial metadata references.

## Security and privacy implications
Input is untrusted, bounded to 1 MiB in the foundation, never logged, and copied rather than rendered as code. Absolute paths are excluded from reports.

## Observability requirements
Report IDs, byte counts, and operation outcomes only. Never log source text.

## Acceptance criteria
A fixture imports offline; its IDs repeat across fresh workspaces; the object digest verifies; valid evidence can be persisted and invalid ranges are rejected.

## Test strategy
Unit and property tests for IDs/ranges; temporary-directory integration tests for import/idempotency; CLI tests for valid and rejected files.

## Deferred decisions
MIME detection, binary artifacts, source revisions, symlink policy configuration, larger-file streaming, and garbage collection.
