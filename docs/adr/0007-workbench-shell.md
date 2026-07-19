# ADR 0007: Build a reusable workbench shell inside the monolith

- Status: Accepted
- Date: 2026-07-17

## Context
LoreMesh needs a reusable terminal workbench shell that can present focused timelines, contextual details, persistent command input, and structured table, metric, and diagram views. The shell must stay generic enough to support current workbench views and future terminal surfaces without turning `loremesh-tui` into a general-purpose framework.

## Decision
Implement provider-neutral shell state, slash-command parsing, focus, theme roles, structured active-view content, and command-handler contracts in `loremesh-tui`. Keep LoreMesh data projection, filesystem writes, service status, and report generation in the application composition root. Do not publish or extract a general framework crate until a second real consumer demonstrates stable boundaries.

## Consequences
The shell is reusable and testable without terminal or storage dependencies, while LoreMesh-specific concepts stay outside its command infrastructure. A synchronous handler is sufficient for the current local workflows; future long-running or remote adapters will require explicit cancellation and orchestration support. Extraction remains possible, but only after a second real consumer proves the boundary is stable.
