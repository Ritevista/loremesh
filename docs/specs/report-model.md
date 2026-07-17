# Report model

## Status
Accepted for the foundation.

## Problem
The same investigation result must render in the TUI and portable files without coupling domain data to a renderer.

## Goals
Provide sections, tables, metrics, and paragraphs; export deterministic JSON, CSV, Markdown, and self-contained escaped HTML.

## Non-goals
PDF, SVG/PNG, interactive HTML, chart drawing, templates/themes, or stable v1 schema.

## User scenarios
An engineer views a workspace report and exports it for review. A table can be opened as CSV without losing row/column consistency.

## Functional requirements
A report has ID, title, and ordered sections. Sections contain ordered blocks. Tables require unique non-empty columns and equal-width rows. Metrics have a label, display value, and optional unit. JSON contains the whole model; Markdown and HTML preserve order; CSV exports the first table and fails if none exists. HTML is self-contained and escapes all data. Output is deterministic and atomically replaces only the named file.

## Domain model
`Report`, `ReportSection`, `ReportBlock`, `TableModel`, and `Metric`. `SavedView` is a named scoped view independent of renderers.

## Interfaces
`render_json`, `render_csv`, `render_markdown`, and `render_html` return bytes/text or typed rendering errors. CLI format enum rejects unknown values.

## Invariants
Titles/labels are non-blank; tables are rectangular; section and row order is stable; renderers do not read storage or network.

## Failure modes
Invalid table shape, empty title, serialization failure, unsupported/no-table CSV, unsafe output path, and I/O error yield non-zero CLI exits without partial success claims.

## Security and privacy implications
HTML escapes untrusted input; shared exports omit absolute paths and personal feedback by default; no scripts or remote assets are emitted.

## Observability requirements
Record format, report ID, destination, and byte count, never report body.

## Acceptance criteria
One structured demo report drives TUI projections and all four exports; JSON parses; CSV is rectangular; Markdown/HTML match reviewed golden files; malicious markup is escaped in HTML.

## Test strategy
Unit/property tests for rectangular tables and round trips; golden tests for Markdown/HTML; CLI integration tests for generated files and invalid formats.

## Deferred decisions
Schema versioning, multiple CSV tables, redaction profiles, SVG/PNG/chart rendering, templates, and streaming large reports.
