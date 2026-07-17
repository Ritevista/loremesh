# Security and privacy

LoreMesh defaults to offline operation and has no telemetry. Future network access requires a named adapter, explicit configuration, and user intent. Source content must not leave the machine merely because a provider is configured.

Imported bytes are untrusted. The foundation canonicalizes inputs, rejects non-files and oversized files, copies bytes into content-addressed objects, validates evidence ranges, prevents workspace-relative traversal, and escapes untrusted report HTML. Exports use logical artifact names rather than absolute host paths. Errors and tracing events include identifiers and operations, not source content or credentials.

SQLite and exports inherit local filesystem permissions. LoreMesh does not provide encryption at rest in this phase; users must rely on OS/disk controls. Personal overlays remain separately scoped in storage. Test fixtures contain generic invented content and no secrets.

Engine and renderer subprocess adapters must use versioned structured messages, fixed executable configuration (never a shell), timeouts, bounded stdin/stdout/stderr, cancellation, and explicit content disclosure. User-requested local shell execution is a separate application boundary: disabled at startup, explicitly enabled per session, visibly untrusted, time/output bounded, and never promoted to evidence automatically. Resource limits, stronger platform sandboxing, export redaction profiles, and malicious database recovery remain tracked risks.
