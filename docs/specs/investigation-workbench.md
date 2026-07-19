# Evidence-driven investigation workbench

## Status

Accepted for the foundation vertical slice.

## Problem

LoreMesh can import, index, trace, and report canonical knowledge, but it cannot persist a user's curated investigation across sessions. Investigation must remain useful offline when no LLM, embedding service, or Graphify runtime is present.

## Goals

Provide a LoreMesh-native investigation that collects stable references to canonical knowledge, preserves personal or organization scope, supports a small explicit review lifecycle, exposes source lineage and evidence currency, stores separate investigation notes, and produces a deterministic self-contained HTML report through the shared report model.

## Non-goals

LLM, Ollama, OpenAI, embeddings, RAG, semantic search, Graphify execution, connectors, binary document import, PDF export, complex graph visualization, RBAC, synchronization, or organization-server collaboration are excluded. Investigation does not author or replace artifacts, findings, claims, evidence, relationships, traces, code references, or feedback.

## User scenarios

An engineer creates a personal investigation, searches the disposable lexical index, collects a canonical artifact hit, adds canonical findings or relationships by ID, reviews deterministic lineage and relationship provenance, records a private investigation note, saves and reopens the investigation, explicitly advances its status, and exports one portable HTML file. A historical finding continues to cite its original snapshot while the report neutrally indicates that a newer source snapshot exists.

## Functional requirements

- Create, list, open, show, save, trace, note, change status, add the current search/opened artifact, add a supported canonical ID, remove an item, and export the open investigation.
- Store stable LoreMesh identifiers, never Tantivy document identifiers or copied canonical object bodies.
- Support `Personal` and `Organization` scope. `SourceDerived` is invalid for investigations.
- Support `Draft`, `InReview`, `Reviewed`, and `Archived` with explicit validated transitions.
- Deduplicate item references while retaining deterministic insertion order.
- Keep investigation notes distinct from canonical feedback. Notes explain the curation/work process and belong to the investigation; feedback reviews a canonical LoreMesh target and retains its own scope and lifecycle.
- Resolve findings, claims, evidence, relationships, traces, code references, snapshots, and sources from canonical storage for views and reports.
- Classify evidence as `Current`, `Historical`, or `Missing`. Historical evidence remains attached to its original artifact and snapshot and is never redirected.
- Preserve relationship origin, verification state, and optional external provider provenance without invoking that provider.
- Build exports as `Investigation -> InvestigationReportBuilder -> Report -> renderer`.
- HTML requires no JavaScript or remote assets, escapes all values, and exposes logical source locations rather than absolute host paths.

## Domain model

`Investigation` contains `InvestigationId`, title, description, `InvestigationScope`, `InvestigationStatus`, ordered unique `InvestigationItem` references, and bounded `InvestigationNote` values. Supported item references are artifacts, findings, claims, evidence references, relationships, traces, and code references already modeled by LoreMesh. The aggregate is curated state, not authority for referenced knowledge.

`EvidenceStatus::{Current, Historical, Missing}` is a query result, not stored evidence state. It compares the evidence artifact's immutable snapshot with the source's current snapshot.

## Interfaces

Core constructors and mutators enforce aggregate invariants. `LocalRepository` saves, lists, loads, and validates investigation references in the existing SQLite workspace. The application command handler owns only the currently open investigation and selected canonical artifact. `InvestigationReportBuilder` receives a resolved, storage-independent report input and creates the shared `Report` model; renderers remain unaware of SQLite or TUI state.

The initial TUI commands are:

```text
/investigation new [--scope personal|organization] <title>
/investigation list
/investigation open <id>
/investigation add current
/investigation add <artifact|finding|claim|relationship|trace|code> <id>
/investigation remove <artifact|finding|claim|relationship|trace|code> <id>
/investigation show
/investigation trace
/investigation note <text>
/investigation status <draft|in-review|reviewed|archived>
/investigation save
/investigation export --format html --output <workspace-relative-path>
```

## Invariants

Titles and notes are non-blank and bounded. Scope never changes implicitly. Items are unique stable canonical references. Saving fails for an unknown reference. Lifecycle changes occur only through the transition method. Investigation persistence is independent of disposable indexes. Canonical objects and historical evidence are never copied, rewritten, or redirected. Personal notes and feedback never become organization knowledge implicitly.

## Failure modes

Blank or oversized values, unsupported scope/status, invalid transition, malformed or unknown typed ID, missing canonical target, missing current investigation, unavailable trace, missing evidence artifact/snapshot/source, non-ready lexical index, unsafe export path, serialization/database error, and report rendering error fail explicitly without partial success claims.

## Security/privacy implications

Investigations may contain private knowledge and remain local unless the user explicitly exports them. No operation uploads content or accesses the network. Imported text is untrusted and is never executed. HTML escaping is mandatory. Reports omit workspace roots, absolute host paths, credentials, and tokens. Personal overlays remain distinguishable and local by default.

## Acceptance criteria

A user can complete search, discover, collect, trace, review, save, reopen, and HTML-export entirely offline. Search collection stores `ArtifactId`. Investigation data survives process and index deletion/rebuild. Reports include metadata, counts, collected objects, findings, claims, evidence with currency, relationships with provenance, source lineage, and relevant feedback/review state. Graphify-derived accepted relationships remain readable when Graphify is absent. HTML is deterministic, self-contained, escaped, and path-safe.

## Test strategy

Core unit tests cover creation, empty title, scope, deduplication, and lifecycle transitions. Storage tests cover reference validation, save/reload, index independence, lineage, evidence currency, relationship provenance, and scoped feedback persistence. Report tests cover deterministic HTML, escaping, required sections, provenance, historical evidence, and absence of host paths. Command parser/handler tests cover state transitions without an interactive terminal. Existing workspace, corpus, index, architecture, lint, documentation, dependency-policy, and public-corpus checks remain required.

## Deferred decisions

Future AI may consume an evidence bundle and propose summaries, explanations, findings, comparisons, gaps, or recommendations. Its output must be validated into LoreMesh findings, claims, and evidence references; it never owns final document structure. Structured LoreMesh objects feed the shared report model and deterministic renderers. PDF should later be an optional `structured Report -> self-contained HTML -> HTML-to-PDF adapter`, not a second report model.

Future PDF, DOCX, PPTX, and other binary ingestion must preserve the original artifact, a normalized representation, and precise provenance back to the original. Identity generation beyond deterministic foundation IDs, note editing/deletion, richer review authorship, export redaction profiles, promotion, RBAC, synchronization, and graphical traces remain deferred.
