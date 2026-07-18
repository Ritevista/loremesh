# Lexical index

## Status

Accepted for the initial local adapter.

## Problem

Scanning a 1–2 GB corpus per query is impractical, but an index must never become the authoritative knowledge store.

## Goals

Provide a replaceable core port, a disposable Tantivy adapter, deterministic rebuilds, explicit lifecycle status, and search hits that resolve to canonical typed artifact and snapshot IDs.

## Non-goals

Semantic search, embeddings, fuzzy ranking guarantees, distributed indexes, remote search, query-language compatibility promises, or indexing private feedback by default.

## User scenarios

An engineer builds a knowledge index, searches titles/bodies/headings/tags, deletes the index directory, rebuilds it from immutable artifacts, and receives the same canonical identities.

## Functional requirements

The port supports build/rebuild, remove, search, status, and drop. Documents contain artifact/source/snapshot IDs, title, body, headings, document/source types, and tags. Search returns typed IDs, score, and a bounded escaped excerpt. States are `NotBuilt`, `Building`, `Ready`, `Stale`, and `Failed`. `loremesh index build knowledge`, `index status`, `index search <query>`, and `index drop knowledge` operate only on `.loremesh/indexes/knowledge`.

## Domain model

`IndexDocument`, `SearchQuery`, `SearchHit`, `IndexBuildResult`, `IndexStatus`, and `LexicalIndex` belong to core as vendor-neutral boundary types. Tantivy schema/documents belong to the storage adapter. Code specifications use the knowledge index; actual source files use a separately named code index boundary.

## Interfaces

The synchronous `LexicalIndex` port accepts validated documents and queries. The composition root reads canonical content through storage and supplies documents to the adapter. The index never calls storage back or mutates canonical records.

## Invariants

Every hit contains parseable canonical IDs; index contents are reconstructible from source snapshots; dropping an index removes no object/database row; failed rebuild leaves canonical knowledge intact; query and result limits are bounded; no network is used.

## Failure modes

Absent/corrupt index, schema mismatch, malformed document, invalid identifier, I/O failure, excessive query, and failed commit return typed index errors. Corruption recommends rebuild rather than modifying knowledge.

## Security and privacy implications

Index files contain local source text and inherit workspace permissions. Queries, bodies, excerpts, and host paths are not logged. Excerpts are bounded and treated as untrusted presentation text.

## Observability requirements

Status exposes state, document count, schema version, and last failure category, not indexed content or queries.

## Acceptance criteria

The tiny corpus builds offline, searches return expected artifact IDs, malformed content fails safely, dropping/rebuilding preserves canonical counts and findings, and disabled/not-built status is actionable.

## Test strategy

Run adapter contract tests in temporary directories, fixture searches for stable terms, rebuild/drop tests, malformed ID tests, Unicode queries, and canonical-storage count comparisons.

## Deferred decisions

Incremental background indexing, snippets with evidence offsets, stemming/language selection, separate code-index implementation, index encryption, large-corpus benchmarks, and stable ranking.
