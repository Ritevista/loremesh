# ADR 0010: Canonical corpus with disposable Tantivy indexes

- Status: Accepted
- Date: 2026-07-18

## Context

LoreMesh must search corpora approaching 1–2 GB and ingest external relationship analysis while preserving evidence and feedback if any engine or index is removed. SQLite metadata and immutable object files already form the authoritative workspace. Making a search engine or Graphify schema canonical would violate replaceability.

## Decision

Keep sources, immutable snapshots, artifacts, evidence, accepted LoreMesh relationships, findings, traces, and feedback canonical. Treat lexical/code indexes and unaccepted external analysis as derived and disposable. Define vendor-neutral `LexicalIndex` and relationship-provider boundary types in `loremesh-core`. Use Tantivy as the initial local knowledge-index adapter in `loremesh-storage`, stored below `.loremesh/indexes/knowledge`. SQLite does not serve as the canonical full-text representation. Translate every external engine object into a LoreMesh relationship candidate before validation; persisted relationships and feedback use LoreMesh IDs.

Tantivy is mature, local, Rust-native, and designed for larger full-text corpora. It adds compile time and a significant transitive dependency surface, so it remains an outward adapter governed by dependency-policy, audit, rebuild, and corruption tests. Alternative adapters such as disabled or SQLite FTS implementations remain possible.

## Consequences

Index deletion and engine replacement cannot destroy knowledge or human review. Search can scale beyond database scans and return canonical IDs. Rebuild orchestration must read and verify immutable artifacts, and index schema changes may require a full rebuild. Local index files contain source text and require the same filesystem protection as the workspace. Separate code indexing and background incremental builds remain deferred.
