# Dependency rules

Dependencies flow inward: binary and UI may depend on report/storage/core; storage and report depend only on core; core depends only on focused data/error crates. Core cannot depend on Ratatui, Crossterm, Rusqlite, filesystem walkers, network clients, Tokio, or vendor SDKs. Storage cannot depend on UI or the binary. Report cannot depend on UI, storage, or the binary.

Workspace manifests make this visible and `scripts/check-architecture.sh` rejects forbidden core dependencies and production panic macros. Unsafe code is forbidden by workspace lint. Clippy's `all` and `pedantic` groups are warnings promoted to errors in CI, with complexity and module-name lints narrowly configured. Missing public-item documentation is not globally enforced while the API is pre-stable; rustdoc warnings are still denied and full enforcement is expected before v1. Exceptions require a narrowly scoped ADR, owner, removal criteria, and code comment.

Current foundation dependencies are documented in ADR 0002; Tantivy's outward adapter role is documented in ADR 0010. Additions require role, feature selection, license, maintenance, and transitive-risk review.
