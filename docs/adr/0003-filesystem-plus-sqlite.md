# ADR 0003: Content-addressed filesystem plus SQLite metadata

- Status: Accepted
- Date: 2026-07-17

## Context
Snapshots can be large immutable bytes, while metadata needs constraints and queries. Neither a loose JSON tree nor database blobs alone serves both cleanly.

## Decision
Store authoritative snapshot bytes beneath `.loremesh/objects/<sha256>` and metadata/derived records in `.loremesh/loremesh.db`. Enable foreign keys, use transactions, and keep a small schema version. Verify digests on read. SQLite is an adapter behind repository behaviour.

## Consequences
Workspaces are inspectable and portable, and metadata operations are transactional. Two storage layers require careful ordering and orphan cleanup; a failed transaction may leave an unreferenced immutable object, which is safe and later collectible. SQLite corruption recovery, migrations, locking, and garbage collection are deferred.
