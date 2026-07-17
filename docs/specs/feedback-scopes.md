# Feedback scopes

## Status
Accepted domain foundation; persistence UI is deferred.

## Problem
Private corrections must not silently change shared knowledge.

## Goals
Model personal, organization, and source-derived knowledge scopes and validate feedback scope/target/status.

## Non-goals
Accounts, permissions, synchronization, moderation UI, conflict resolution, or promotion workflow.

## User scenarios
An engineer records a private correction; a future reviewer explicitly creates a distinct organization correction without rewriting the personal record.

## Functional requirements
Feedback targets an artifact, finding, claim, or trace edge. Feedback accepts only `Personal` or `Organization`; `SourceDerived` is rejected. Feedback never mutates the target. Verification status is explicit.

## Domain model
`KnowledgeScope::{Personal, Organization, SourceDerived}`, typed `FeedbackId`, `FeedbackTarget`, text, and `VerificationStatus`.

## Interfaces
Core `Feedback::new` validates scope and non-blank bounded text. Future repositories store scope as a required enum, never a nullable flag.

## Invariants
Personal and organization records have distinct IDs and rows. Promotion cannot be inferred from status. Rejected/disputed feedback remains auditable.

## Failure modes
Invalid source-derived feedback, blank/oversized text, unknown target, or unknown persisted enum fails explicitly.

## Security and privacy implications
Personal data is excluded from shared exports by default. No implicit promotion or upload is allowed.

## Observability requirements
Logs may include feedback ID and scope but not text.

## Acceptance criteria
Constructors accept both permitted scopes, reject `SourceDerived`, preserve target/status, and serialize enums without ambiguous strings.

## Test strategy
Unit tests for scope/text invariants and serialization round trips; future adapter contract tests verify scoped queries.

## Deferred decisions
Identity, access control, signatures, promotion review, deletion, synchronization, and conflict policy.
