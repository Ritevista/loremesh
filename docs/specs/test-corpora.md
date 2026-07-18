# Test corpora

## Status

Accepted for tiny, public, and scale corpus profiles.

## Problem

Private engineering material cannot be committed, while unit-only toy strings do not exercise realistic lineage, health, search, and scale behavior.

## Goals

Provide a committed deterministic fixture, an explicitly downloaded pinned public corpus, and a deterministic scalable synthetic generator with distinct CI and licensing policies.

## Non-goals

Mirroring all upstream history, benchmarking in normal CI, proprietary examples, generated natural-language realism from an LLM, or treating expected relationships as canonical production truth.

## User scenarios

A contributor runs `just corpus-fixture` in CI, explicitly runs `just corpus-public` when network access is acceptable, or creates bounded 100 MB through 2 GB synthetic corpora for manual profiling.

## Functional requirements

The tiny fixture contains fictional documents, issues, a small Rust service, code references, images/placeholders, expected diagnostics, and known relationships. The initial public profile transforms KEP 753, 1287, 2579, and 3294 from `kubernetes/enhancements` commit `996e7d41387c4937b0b976800718d067cd1bdd16` and code from Kubernetes v1.34.0 commit `f28b4c9efbca5c5c0af716d9f2d5702667ee8a45`; both upstreams are Apache-2.0 and their immutable license URLs are recorded in the profile/output. It writes below `target/test-corpora` and never runs in ordinary tests. The scale generator accepts seed, counts, relationships, target size, output, and an optional deliberate-quality-problems mode; it refuses unsafe/existing output, prints the target and requested size before generation, and produces logically equivalent output for equal version/arguments.

## Domain model

All three outputs use the same schema-versioned corpus manifest. Expected files are evaluation truth only. Generator metadata records generator version, seed, and normalized arguments.

## Interfaces

`just corpus-fixture`, `just corpus-public`, `just corpus-scale-100m`, `just corpus-scale-500m`, `just corpus-scale-1g`, `just corpus-scale-2g`, and `just corpus-clean` are explicit entry points. Large commands require an affirmative size flag in the underlying tool.

## Invariants

Tiny tests are offline and small; public inputs are pinned and license-attributed; generated content is fictional; outputs stay ignored below `target/test-corpora`; equal inputs preserve logical identities and relationships; no generated large corpus is committed.

## Failure modes

Missing network/tooling, upstream revision mismatch, license metadata omission, insufficient disk, interrupted generation, unsafe output, duplicate seed identity, and invalid requested sizes fail with partial-output guidance.

## Security and privacy implications

Only the public builder may use network access and only after explicit invocation. Imported code is never executed. Synthetic templates contain no real people, credentials, organizations, or internal names.

## Observability requirements

Tools print profile, revision, destination, requested/actual byte counts, record counts, duration, and failure category; never environment credentials or host paths in portable manifests.

## Acceptance criteria

Tiny fixture tests run in CI; public builder supports a documented dry-run/manifest verification without network; scale generator produces a small CI test corpus deterministically and exposes guarded 100 MB–2 GB recipes.

## Test strategy

CI validates fixture licenses/content, runs a tiny generator golden comparison, validates public profile pins without fetching, and checks generated manifests. Public fetches and large generation remain manual.

## Deferred decisions

Additional upstream projects, archive caching, benchmark harness integration, evaluation metrics, binary attachment generation, and automated scheduled performance workflows.
