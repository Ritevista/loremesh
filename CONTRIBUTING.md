# Contributing

Thank you for improving LoreMesh. Discuss substantial behaviour in an issue and update or add a specification before implementation. Record durable architecture decisions in an ADR. Use conventional commits (`type(scope): summary`), such as `feat(storage): import immutable snapshots`.

Fork the repository, create a focused branch, run `just ci`, and submit a pull request using the template. New dependencies need a documented role, license review, maintenance assessment, and minimal feature set. Defect fixes require a regression test. Public formats need compatibility notes and migration behaviour.

Corpus changes must remain fictional or public and license-attributed. Run `just corpus-fixture` for importer/index changes and `just corpus-public-verify` for public-profile changes. Do not commit output from `target/test-corpora`, invoke public downloads in ordinary tests, or generate scale profiles in CI.

By participating, you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Security reports belong in the private process described by [SECURITY.md](SECURITY.md), not public issues.
