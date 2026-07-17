# Terminology

- **Workspace:** local directory containing LoreMesh metadata and derived state.
- **Source:** configured origin of imported material; foundation sources are local files.
- **Source snapshot:** immutable observation of a source, identified by content digest.
- **Artifact:** typed, addressable material exposed by a snapshot.
- **Evidence reference:** a byte range within an artifact used to support a claim.
- **Claim:** one evidence-backed assertion.
- **Finding:** reviewable collection of claims about an investigation.
- **Source lineage:** relationship from derived knowledge to authoritative snapshots.
- **Processing lineage:** record of transformations or decisions that created derived knowledge.
- **Trace:** directed graph connecting findings, evidence, snapshots, and processing steps.
- **Feedback:** scoped correction or annotation; never an implicit mutation of its target.
- **Report:** renderer-independent sections, tables, and metrics.
- **Saved view:** named, scoped dashboard query state.
- **Graph engine:** optional replaceable analysis system behind a port.
