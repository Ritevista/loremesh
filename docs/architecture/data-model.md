# Data model

Typed identifiers prevent accidental cross-entity references. A workspace contains sources; a source has immutable snapshots; an artifact identifies content exposed by one snapshot. Evidence references a validated half-open byte range `[start, end)` in an artifact. A finding contains one or more claims, and each claim contains evidence. A trace is a directed acyclic graph whose nodes identify finding, evidence, snapshot, or processing-step concepts and whose edges declare origin and verification state.

Feedback points to a target and carries `Personal` or `Organization` scope. `SourceDerived` is valid for knowledge created from sources but invalid for feedback authorship. Reports contain sections composed of paragraphs, tables, and metrics. Saved views store a name and explicit scope only in the foundation.

Identifiers use stable prefixed SHA-256-derived strings for imported content and deterministic demo entities. User-authored production IDs may later use a monotonic generator behind an injected port. Timestamps are stored only when externally meaningful and are injected; the foundation avoids them for deterministic state.
