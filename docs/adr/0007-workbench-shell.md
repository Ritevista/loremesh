# ADR 0007: Build a reusable workbench shell inside the monolith

- Status: Accepted
- Date: 2026-07-17

## Context
The four-quadrant foundation dashboard proves projection correctness but gives every concern equal visual weight. LoreMesh needs a focused investigation timeline, contextual details, persistent command input, and reusable table/metric/diagram presentation. Similar mechanics may later support other terminal workbenches, but no second product exists yet.

## Decision
Implement provider-neutral shell state, slash-command parsing, focus, theme roles, structured active-view content, and command-handler contracts in `loremesh-tui`. Keep LoreMesh data projection, filesystem writes, service status, and report generation in the application composition root. Do not publish or extract a general framework crate until a second real consumer demonstrates stable boundaries.

## Consequences
The shell is reusable and testable without terminal or storage dependencies, while LoreMesh-specific concepts stay outside its command infrastructure. A synchronous handler is sufficient for local foundation commands; future long-running adapters will require cancellable task orchestration. Extraction remains possible but is evidence-driven.
