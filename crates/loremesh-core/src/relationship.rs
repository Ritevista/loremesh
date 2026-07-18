//! Canonical relationships and external-provider boundary models.

use serde::{Deserialize, Serialize};

use crate::{
    non_blank, ArtifactId, CodeReferenceId, DomainError, EdgeOrigin, EvidenceReference,
    RelationshipId, SnapshotId, SourceId, VerificationStatus,
};

/// Durable reference into a pinned source-code repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeReference {
    pub id: CodeReferenceId,
    pub repository: String,
    pub revision: String,
    pub path: String,
    pub symbol: Option<String>,
    pub line_range: Option<(u64, u64)>,
}

impl CodeReference {
    pub fn new(
        id: CodeReferenceId,
        repository: impl Into<String>,
        revision: impl Into<String>,
        path: impl Into<String>,
        symbol: Option<String>,
        line_range: Option<(u64, u64)>,
    ) -> Result<Self, DomainError> {
        let repository = repository.into();
        let revision = revision.into();
        let path = path.into();
        non_blank("code repository", &repository)?;
        non_blank("code revision", &revision)?;
        non_blank("code path", &path)?;
        if path.starts_with('/') || path.contains("..") || path.contains('\\') {
            return Err(DomainError::Validation {
                field: "code path",
                reason: "must be a safe repository-relative path".into(),
            });
        }
        if let Some((start, end)) = line_range {
            if start == 0 || start > end {
                return Err(DomainError::Validation {
                    field: "code line range",
                    reason: "must satisfy 1 <= start <= end".into(),
                });
            }
        }
        if symbol.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err(DomainError::Validation {
                field: "code symbol",
                reason: "must not be blank when present".into(),
            });
        }
        Ok(Self {
            id,
            repository,
            revision,
            path,
            symbol,
            line_range,
        })
    }
}

/// Typed endpoint accepted by a canonical relationship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum RelationshipEndpoint {
    Artifact(ArtifactId),
    Source(SourceId),
    Snapshot(SnapshotId),
    Code(CodeReferenceId),
}

impl RelationshipEndpoint {
    fn canonical(&self) -> String {
        match self {
            Self::Artifact(id) => format!("artifact:{id}"),
            Self::Source(id) => format!("source:{id}"),
            Self::Snapshot(id) => format!("snapshot:{id}"),
            Self::Code(id) => format!("code:{id}"),
        }
    }
}

/// Extensible relation vocabulary with a stable portable spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelationType(String);

impl RelationType {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        non_blank("relation type", &value)?;
        if value.len() > 96
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._:-".contains(&byte)
            })
        {
            return Err(DomainError::Validation {
                field: "relation type",
                reason: "must be at most 96 lowercase ASCII letters, digits, '.', '_', ':', or '-'"
                    .into(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Optional metadata describing an external analysis run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalProvenance {
    pub provider: String,
    pub provider_version: String,
    pub run_id: String,
    pub configuration_digest: String,
    pub observed_at: Option<String>,
    pub external_id: Option<String>,
}

impl ExternalProvenance {
    pub fn validate(&self) -> Result<(), DomainError> {
        for (field, value) in [
            ("provider", self.provider.as_str()),
            ("provider version", self.provider_version.as_str()),
            ("provider run ID", self.run_id.as_str()),
        ] {
            non_blank(field, value)?;
        }
        if self.configuration_digest.len() != 64
            || !self
                .configuration_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(DomainError::Validation {
                field: "provider configuration digest",
                reason: "must be 64 lowercase hexadecimal characters".into(),
            });
        }
        if self
            .external_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err(DomainError::Validation {
                field: "external relationship ID",
                reason: "must be non-blank and at most 256 bytes when present".into(),
            });
        }
        Ok(())
    }
}

/// LoreMesh-owned relationship that remains meaningful without its provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: RelationshipId,
    pub source: RelationshipEndpoint,
    pub relation: RelationType,
    pub target: RelationshipEndpoint,
    pub origin: EdgeOrigin,
    pub status: VerificationStatus,
    pub evidence: Vec<EvidenceReference>,
    pub external_provenance: Option<ExternalProvenance>,
}

impl Relationship {
    pub fn new(
        source: RelationshipEndpoint,
        relation: RelationType,
        target: RelationshipEndpoint,
        origin: EdgeOrigin,
        status: VerificationStatus,
        evidence: Vec<EvidenceReference>,
        external_provenance: Option<ExternalProvenance>,
    ) -> Result<Self, DomainError> {
        if source == target {
            return Err(DomainError::Validation {
                field: "relationship endpoints",
                reason: "source and target must differ".into(),
            });
        }
        if let Some(provenance) = &external_provenance {
            provenance.validate()?;
        }
        let identity = format!(
            "{}|{}|{}",
            source.canonical(),
            relation.as_str(),
            target.canonical()
        );
        Ok(Self {
            id: RelationshipId::deterministic(identity),
            source,
            relation,
            target,
            origin,
            status,
            evidence,
            external_provenance,
        })
    }
}

/// Unaccepted output translated from any relationship provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipCandidate {
    pub source: RelationshipEndpoint,
    pub relation: RelationType,
    pub target: RelationshipEndpoint,
    pub origin: EdgeOrigin,
    pub evidence: Vec<EvidenceReference>,
    pub provenance: ExternalProvenance,
}

impl RelationshipCandidate {
    pub fn validate(self) -> Result<Relationship, DomainError> {
        Relationship::new(
            self.source,
            self.relation,
            self.target,
            self.origin,
            VerificationStatus::Unreviewed,
            self.evidence,
            Some(self.provenance),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Feedback, FeedbackId, FeedbackTarget, KnowledgeScope};

    fn provenance(provider: &str, external_id: &str) -> ExternalProvenance {
        ExternalProvenance {
            provider: provider.into(),
            provider_version: "1.0.0".into(),
            run_id: "run-1".into(),
            configuration_digest: "a".repeat(64),
            observed_at: None,
            external_id: Some(external_id.into()),
        }
    }

    #[test]
    fn provider_identity_does_not_control_relationship_or_feedback_identity() {
        let source = RelationshipEndpoint::Artifact(ArtifactId::deterministic("source"));
        let target = RelationshipEndpoint::Artifact(ArtifactId::deterministic("target"));
        let relation = RelationType::parse("implements").expect("relation");
        let first = Relationship::new(
            source.clone(),
            relation.clone(),
            target.clone(),
            EdgeOrigin::Extracted,
            VerificationStatus::Unreviewed,
            Vec::new(),
            Some(provenance("engine-a", "edge-7")),
        )
        .expect("first relationship");
        let replacement = Relationship::new(
            source,
            relation,
            target,
            EdgeOrigin::Inferred,
            VerificationStatus::Unreviewed,
            Vec::new(),
            Some(provenance("engine-b", "object-99")),
        )
        .expect("replacement relationship");
        assert_eq!(first.id, replacement.id);
        let feedback = Feedback::new(
            FeedbackId::deterministic("review"),
            FeedbackTarget::Relationship(first.id.clone()),
            KnowledgeScope::Personal,
            "This link is incorrect.",
            VerificationStatus::Disputed,
        )
        .expect("relationship feedback");
        assert_eq!(
            feedback.target,
            FeedbackTarget::Relationship(replacement.id)
        );
    }

    #[test]
    fn code_reference_requires_revision_and_safe_path() {
        let result = CodeReference::new(
            CodeReferenceId::deterministic("bad"),
            "repo",
            "",
            "../secret",
            None,
            None,
        );
        assert!(result.is_err());
    }
}
