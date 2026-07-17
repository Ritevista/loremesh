# Testing strategy

Pure unit tests cover identifiers, evidence ranges, scope rules, traces, and report transformations. Storage integration and adapter contract tests run in isolated temporary directories. CLI tests assert status, output, diagnostics, and generated files. TUI logic is represented by pure view models so tests need no terminal. Markdown and HTML outputs use small checked-in golden fixtures. Property tests target identifier validation, evidence ranges, trace paths, and serialization/table consistency.

Tests are offline, deterministic, order-independent, license-safe, and may use `expect` for diagnostic quality. Time and ID generation must be injected when introduced. Temporary paths never escape their owning directory. Every defect fix adds a regression test.

CI runs formatting, check, Clippy with denied warnings, all tests, doctests, docs with warnings denied, architecture rules, dependency policy, audit, unused-dependency analysis, and coverage generation. Coverage is published without a threshold until a meaningful baseline exists; later policy should prevent unexplained regression. Linux is required; macOS and Windows run the portable build/test subset.
