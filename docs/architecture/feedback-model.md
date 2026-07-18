# Feedback model

Feedback is an overlay on a finding, claim, artifact, trace edge, or LoreMesh-native relationship. It has its own identity, target, scope, text, and verification status. It does not rewrite its target. External provider identifiers may appear in relationship provenance but are never feedback targets.

Personal feedback is private local state and cannot be promoted implicitly. Organization feedback represents an explicitly reviewed shared correction. `SourceDerived` describes derived knowledge and is invalid as a feedback scope. Promotion, conflict resolution, identities, signatures, and synchronization are deferred; a future promotion must create a new organization record with auditable provenance and user confirmation.
