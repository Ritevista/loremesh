# ADR 0006: Apache-2.0 OR MIT licensing

- Status: Accepted
- Date: 2026-07-17

## Context
Rust projects commonly offer a permissive dual-license compatible with broad commercial and open-source use.

## Decision
License contributions and distributions under Apache License 2.0 or MIT, at the recipient's option. Package metadata uses `Apache-2.0 OR MIT`. Contributions are made under the same terms unless explicitly stated.

## Consequences
Users choose either license. Maintainers must keep both license texts, review dependency compatibility, and avoid incompatible copied material. A contributor agreement is not required today.
