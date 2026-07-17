# ADR 0001: Begin as a modular monolith

- Status: Accepted
- Date: 2026-07-17

## Context
The product needs clear replaceable boundaries but has one local user, one process, and no demonstrated distributed scaling need.

## Decision
Build one Rust workspace and application process with five focused crates. The binary is the composition root. Modules communicate through typed calls and narrow ports; external engines may run as subprocesses.

## Consequences
Development, transactions, debugging, and offline distribution remain simple. Crate dependency rules provide useful enforcement. Components cannot deploy independently, which is not currently valuable. Splitting services requires evidence and a new ADR.
