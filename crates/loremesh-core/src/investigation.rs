//! Persistent curation of canonical `LoreMesh` knowledge references.

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactId, ClaimId, CodeReferenceId, DomainError, EvidenceReference, FindingId,
    InvestigationId, RelationshipId, TraceId,
};

/// Privacy boundary of an investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationScope {
    Personal,
    Organization,
}

/// Explicit review lifecycle of an investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationStatus {
    Draft,
    InReview,
    Reviewed,
    Archived,
}

impl InvestigationStatus {
    fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Draft | Self::Reviewed,
                Self::InReview | Self::Archived
            ) | (
                Self::InReview,
                Self::Draft | Self::Reviewed | Self::Archived
            )
        )
    }
}

/// Stable reference collected by an investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reference", rename_all = "snake_case")]
pub enum InvestigationItem {
    Artifact(ArtifactId),
    Finding(FindingId),
    Claim(ClaimId),
    Evidence(EvidenceReference),
    Relationship(RelationshipId),
    Trace(TraceId),
    CodeReference(CodeReferenceId),
}

/// A private or shared curation note, distinct from canonical feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationNote {
    pub text: String,
}

impl InvestigationNote {
    fn new(text: impl Into<String>) -> Result<Self, DomainError> {
        let text = text.into();
        if text.trim().is_empty() || text.len() > 4096 {
            return Err(DomainError::Validation {
                field: "investigation note",
                reason: "must be non-blank and at most 4096 bytes".into(),
            });
        }
        Ok(Self { text })
    }
}

/// Persistent curated collection of canonical knowledge references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Investigation {
    pub id: InvestigationId,
    pub title: String,
    pub description: String,
    pub scope: InvestigationScope,
    pub status: InvestigationStatus,
    pub items: Vec<InvestigationItem>,
    pub notes: Vec<InvestigationNote>,
}

impl Investigation {
    /// Creates an empty draft investigation.
    pub fn new(
        id: InvestigationId,
        title: impl Into<String>,
        description: impl Into<String>,
        scope: InvestigationScope,
    ) -> Result<Self, DomainError> {
        let title = title.into();
        let description = description.into();
        if title.trim().is_empty() || title.len() > 256 {
            return Err(DomainError::Validation {
                field: "investigation title",
                reason: "must be non-blank and at most 256 bytes".into(),
            });
        }
        if description.len() > 4096 {
            return Err(DomainError::Validation {
                field: "investigation description",
                reason: "must be at most 4096 bytes".into(),
            });
        }
        Ok(Self {
            id,
            title,
            description,
            scope,
            status: InvestigationStatus::Draft,
            items: Vec::new(),
            notes: Vec::new(),
        })
    }

    /// Adds a reference once, returning whether the collection changed.
    pub fn add_item(&mut self, item: InvestigationItem) -> bool {
        if self.items.contains(&item) {
            false
        } else {
            self.items.push(item);
            true
        }
    }

    /// Removes a reference, returning whether the collection changed.
    pub fn remove_item(&mut self, item: &InvestigationItem) -> bool {
        let length = self.items.len();
        self.items.retain(|candidate| candidate != item);
        self.items.len() != length
    }

    /// Adds a bounded curation note.
    pub fn add_note(&mut self, text: impl Into<String>) -> Result<(), DomainError> {
        self.notes.push(InvestigationNote::new(text)?);
        Ok(())
    }

    /// Performs a validated explicit lifecycle transition.
    pub fn transition_to(&mut self, next: InvestigationStatus) -> Result<(), DomainError> {
        if self.status == next {
            return Ok(());
        }
        if !self.status.can_transition_to(next) {
            return Err(DomainError::Validation {
                field: "investigation status",
                reason: format!("cannot transition from {:?} to {next:?}", self.status),
            });
        }
        self.status = next;
        Ok(())
    }
}

/// Currency of immutable evidence relative to current source state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Current,
    Historical,
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_scoped_draft_and_rejects_empty_title() {
        let investigation = Investigation::new(
            InvestigationId::deterministic("alpha"),
            "Alpha",
            "",
            InvestigationScope::Personal,
        )
        .expect("valid investigation");
        assert_eq!(investigation.scope, InvestigationScope::Personal);
        assert_eq!(investigation.status, InvestigationStatus::Draft);
        assert!(Investigation::new(
            InvestigationId::deterministic("empty"),
            "  ",
            "",
            InvestigationScope::Organization
        )
        .is_err());
    }

    #[test]
    fn deduplicates_items_and_requires_explicit_valid_transitions() {
        let mut investigation = Investigation::new(
            InvestigationId::deterministic("alpha"),
            "Alpha",
            "",
            InvestigationScope::Personal,
        )
        .expect("valid investigation");
        let item = InvestigationItem::Artifact(ArtifactId::deterministic("artifact"));
        assert!(investigation.add_item(item.clone()));
        assert!(!investigation.add_item(item));
        assert!(investigation
            .transition_to(InvestigationStatus::Reviewed)
            .is_err());
        investigation
            .transition_to(InvestigationStatus::InReview)
            .expect("review transition");
        investigation
            .transition_to(InvestigationStatus::Reviewed)
            .expect("reviewed transition");
    }
}
