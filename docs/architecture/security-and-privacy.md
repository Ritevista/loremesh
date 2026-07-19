# Security and privacy

LoreMesh defaults to offline operation and has no telemetry. Future external integrations require explicit configuration and user intent. Source content must not leave the machine merely because a provider is configured.

Imported bytes are untrusted. The foundation canonicalizes inputs, rejects non-files and oversized files, copies bytes into content-addressed objects, validates evidence ranges, prevents workspace-relative traversal, and escapes untrusted report HTML. Exports use logical artifact names rather than absolute host paths. Errors and tracing events include identifiers and operations, not source content or credentials.

Corpus manifests and artifact paths are bounded, normalized beneath the manifest root, and reject symlinks and traversal. Import never executes repository code or fetches URLs. The committed fixture is fictional. The public builder is the only corpus tool with network behavior; it must be invoked explicitly, fetches immutable revisions, records upstream Apache-2.0 license URLs, and writes ignored output below `target/test-corpora`. Large synthetic generation requires an explicit acknowledgement.

SQLite and exports inherit local filesystem permissions. LoreMesh does not provide encryption at rest in this phase; users must rely on OS/disk controls. Personal overlays remain separately scoped in storage. Test fixtures contain generic invented content and no secrets.

Engine and renderer subprocess adapters must use versioned structured messages, fixed executable configuration (never a shell), timeouts, bounded stdin/stdout/stderr, cancellation, and explicit content disclosure. User-requested `/shell` execution is a separate application boundary: never started implicitly, scoped to one TUI session, backed by bounded transcript memory, visibly untrusted, and never promoted to evidence automatically. It runs with the user's OS authority and the workspace working directory is not a sandbox. Resource limits, stronger platform sandboxing, export redaction profiles, and malicious database recovery remain tracked risks.
