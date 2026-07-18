# Corpus manifest

## Status

Accepted as pre-stable schema version 1.

## Problem

Fixtures, public-source builders, and scale generators need one vendor-neutral way to describe content and provenance without embedding document bodies in metadata.

## Goals

Define deterministic JSON metadata for corpus identity, sources, artifacts, code references, relationships, expectations, and optional external-analysis imports.

## Non-goals

Canonical database export, arbitrary extension execution, Kubernetes-specific fields, Graphify-native objects, YAML support, or a stable v1 compatibility promise.

## User scenarios

A fixture author lists Markdown and code files with logical identities. A public builder records pinned upstream repositories. An external adapter emits relationship candidates with engine provenance.

## Functional requirements

`corpus.json` contains `schema_version`, corpus `name` and `version`, `sources`, `artifacts`, optional `code_references`, optional `relationships`, optional `expected_relationships`, and optional `external_analyses`. Artifact entries refer to content by safe relative path and never duplicate bytes. Source provenance records a generic kind, origin, and immutable revision where applicable. Unknown schema versions fail. Entries are processed in manifest order but persisted and reported deterministically.

## Domain model

Manifest identities are bounded non-blank strings local to the manifest. Relationship endpoints refer to manifest artifact/source/code-reference identities and are translated into typed LoreMesh IDs. External provenance contains provider name, version, run identifier, configuration digest, and optional externally supplied timestamp; external object IDs are metadata only.

## Interfaces

Serde JSON is the wire format. `CorpusManifest::from_reader` parses with a byte bound; `validate(root, limits)` returns all safe structural diagnostics before mutation. Builders emit generic manifests and ordinary files.

## Invariants

Every identity is unique within its namespace; every content path is relative, normalized, and below the corpus root; referenced identities exist unless deliberately declared as an expected diagnostic; revisions for actual code references are non-blank and immutable; relationship provenance never determines LoreMesh identity.

## Failure modes

Malformed JSON, unsupported schema, unknown fields that change meaning, duplicates, missing targets, unsafe paths, missing revisions, and excessive entries fail or produce explicitly classified diagnostics according to manifest intent.

## Security and privacy implications

Parsing performs no fetching or execution. Absolute paths and parent components are forbidden. Errors expose logical paths only. External timestamps and identifiers are treated as untrusted bounded strings.

## Observability requirements

Report schema version, corpus identity, entry counts, diagnostic codes, and stable logical IDs; never content bodies.

## Acceptance criteria

The committed manifest round-trips deterministically, validates all intended good entries, reports deliberate broken/duplicate candidates, rejects traversal, and is independent of Graphify and Kubernetes types.

## Test strategy

Unit tests cover structural validation and serialization; fixture tests cover every entry category; property tests cover safe relative paths and deterministic translation.

## Deferred decisions

Schema migrations, signatures, archive packaging, YAML, content chunks, remote URI retrieval, and standardized extension namespaces.
