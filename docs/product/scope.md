# Scope

## Foundation scope

The foundation initializes a local workspace, imports Markdown as an immutable snapshot, creates deterministic demo evidence and a manual finding, constructs a trace, shows a minimal TUI, and exports one report to JSON, CSV, Markdown, or self-contained HTML.

The next investigation slice adds a persistent investigation that can collect canonical references from local search, preserve personal or organization scope, inspect evidence and lineage, attach local notes or feedback, save and reopen, and export a self-contained HTML report.

## Non-goals

No remote connectors, collaboration server, live source watching, semantic search, embeddings, LLM integration, Graphify integration, dynamic plugins, authentication, release publishing, rich chart rendering, image import, or migration compatibility promise is included. SQLite is local metadata storage, not an authoritative copy of source content.

## Constraints

Operation is offline by default. Imported files are untrusted and bounded. Personal and organization scopes are distinct. Exports escape content and use workspace-relative paths. Derived state can be rebuilt from sources and explicit user input. Investigations reference canonical knowledge; they do not duplicate the underlying artifacts, findings, claims, evidence, or relationships.
