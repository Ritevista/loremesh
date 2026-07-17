# ADR 0004: Ports and adapters at exercised boundaries

- Status: Accepted
- Date: 2026-07-17

## Context
Storage and future engines must be replaceable, while speculative interfaces would add noise.

## Decision
Core owns domain types and only ports required by current use cases. Concrete filesystem/SQLite and presentation code live outward. Vendor data is translated at adapter boundaries. Dependencies are passed through constructors. The binary selects implementations.

## Consequences
Core tests remain fast and vendor-free, and adapters can have behavioural contract suites. Some orchestration stays in the binary until repeated use cases justify an application crate. New ports need two credible consumers or an exercised external boundary.
