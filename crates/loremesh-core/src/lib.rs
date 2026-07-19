//! Canonical `LoreMesh` domain types and invariants.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{self, Display};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod corpus;
pub mod index;
pub mod investigation;
pub mod relationship;

/// Domain validation and invariant failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// A user-facing value is empty or malformed.
    #[error("validation failed for {field}: {reason}")]
    Validation { field: &'static str, reason: String },
    /// A graph invariant would be violated.
    #[error("trace invariant failed: {0}")]
    TraceInvariant(String),
    /// A requested entity does not exist.
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },
}

fn non_blank(field: &'static str, value: &str) -> Result<(), DomainError> {
    if value.trim().is_empty() {
        return Err(DomainError::Validation {
            field,
            reason: "must not be blank".into(),
        });
    }
    Ok(())
}

macro_rules! identifier {
    ($name:ident, $prefix:literal) => {
        #[doc = concat!("Typed identifier for ", stringify!($name), ".")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Deterministically derives an identifier from canonical bytes.
            pub fn deterministic(seed: impl AsRef<[u8]>) -> Self {
                let digest = Sha256::digest(seed.as_ref());
                Self(format!(concat!($prefix, "_{}"), hex::encode(&digest[..12])))
            }

            /// Parses and validates an identifier.
            pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
                let value = value.into();
                let suffix = value.strip_prefix(concat!($prefix, "_")).ok_or_else(|| {
                    DomainError::Validation {
                        field: "identifier",
                        reason: format!("must start with {}_", $prefix),
                    }
                })?;
                if suffix.len() != 24
                    || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                    || suffix.bytes().any(|byte| byte.is_ascii_uppercase())
                {
                    return Err(DomainError::Validation {
                        field: "identifier",
                        reason: "suffix must be 24 lowercase hexadecimal characters".into(),
                    });
                }
                Ok(Self(value))
            }

            /// Returns the serialized identifier.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(WorkspaceId, "wrk");
identifier!(SourceId, "src");
identifier!(SnapshotId, "snp");
identifier!(ArtifactId, "art");
identifier!(FindingId, "fnd");
identifier!(ClaimId, "clm");
identifier!(FeedbackId, "fbk");
identifier!(TraceId, "trc");
identifier!(TraceNodeId, "nod");
identifier!(TraceEdgeId, "edg");
identifier!(ReportId, "rpt");
identifier!(SavedViewId, "viw");
identifier!(RelationshipId, "rel");
identifier!(CodeReferenceId, "cod");
identifier!(InvestigationId, "inv");

/// Local workspace descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// Stable identity.
    pub id: WorkspaceId,
    /// User-visible name.
    pub name: String,
    /// Runtime-only local root.
    #[serde(skip)]
    pub root: PathBuf,
}

impl Workspace {
    /// Creates a validated workspace.
    pub fn new(
        id: WorkspaceId,
        name: impl Into<String>,
        root: PathBuf,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        non_blank("workspace name", &name)?;
        if root.as_os_str().is_empty() {
            return Err(DomainError::Validation {
                field: "workspace root",
                reason: "must not be empty".into(),
            });
        }
        Ok(Self { id, name, root })
    }
}

/// Configured origin of source material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Stable identity.
    pub id: SourceId,
    /// Workspace-relative logical location.
    pub location: String,
}

impl Source {
    /// Creates a local source with a safe logical location.
    pub fn local(id: SourceId, location: impl Into<String>) -> Result<Self, DomainError> {
        let location = location.into();
        non_blank("source location", &location)?;
        if location.starts_with('/') || location.contains("..") || location.contains('\\') {
            return Err(DomainError::Validation {
                field: "source location",
                reason: "must be a safe logical name, not a host path".into(),
            });
        }
        Ok(Self { id, location })
    }
}

/// Immutable observation of source bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    /// Stable identity.
    pub id: SnapshotId,
    /// Source observed.
    pub source_id: SourceId,
    /// Lowercase SHA-256 digest.
    pub digest: String,
    /// Byte length.
    pub byte_len: u64,
}

impl SourceSnapshot {
    /// Creates a validated immutable snapshot descriptor.
    pub fn new(
        id: SnapshotId,
        source_id: SourceId,
        digest: impl Into<String>,
        byte_len: u64,
    ) -> Result<Self, DomainError> {
        let digest = digest.into();
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(DomainError::Validation {
                field: "snapshot digest",
                reason: "must be 64 lowercase hexadecimal characters".into(),
            });
        }
        Ok(Self {
            id,
            source_id,
            digest,
            byte_len,
        })
    }
}

/// Imported addressable material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable identity.
    pub id: ArtifactId,
    /// Snapshot containing the bytes.
    pub snapshot_id: SnapshotId,
    /// Logical display name.
    pub name: String,
    /// Media type.
    pub media_type: String,
    /// Byte length used to validate evidence.
    pub byte_len: u64,
}

impl Artifact {
    /// Creates a validated artifact.
    pub fn new(
        id: ArtifactId,
        snapshot_id: SnapshotId,
        name: impl Into<String>,
        byte_len: u64,
    ) -> Result<Self, DomainError> {
        Self::with_media_type(id, snapshot_id, name, "text/markdown", byte_len)
    }

    /// Creates a validated artifact with an explicit media type.
    pub fn with_media_type(
        id: ArtifactId,
        snapshot_id: SnapshotId,
        name: impl Into<String>,
        media_type: impl Into<String>,
        byte_len: u64,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        let media_type = media_type.into();
        non_blank("artifact name", &name)?;
        non_blank("artifact media type", &media_type)?;
        Ok(Self {
            id,
            snapshot_id,
            name,
            media_type,
            byte_len,
        })
    }
}

/// Reference to an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    /// Referenced artifact.
    pub artifact_id: ArtifactId,
}

/// Stable evidence range within one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReference {
    /// Referenced artifact.
    pub artifact: ArtifactReference,
    /// Inclusive start byte.
    pub start: u64,
    /// Exclusive end byte.
    pub end: u64,
    /// Concise context label, not copied source content.
    pub label: String,
}

impl EvidenceReference {
    /// Validates an evidence range against artifact content.
    pub fn new(
        artifact: ArtifactReference,
        start: u64,
        end: u64,
        label: impl Into<String>,
        content: &str,
    ) -> Result<Self, DomainError> {
        let label = label.into();
        non_blank("evidence label", &label)?;
        let len = u64::try_from(content.len()).map_err(|_| DomainError::Validation {
            field: "evidence range",
            reason: "content is too large".into(),
        })?;
        if start >= end || end > len {
            return Err(DomainError::Validation {
                field: "evidence range",
                reason: format!("must satisfy 0 <= start < end <= {len}"),
            });
        }
        let start_usize = usize::try_from(start).map_err(|_| DomainError::Validation {
            field: "evidence start",
            reason: "does not fit this platform".into(),
        })?;
        let end_usize = usize::try_from(end).map_err(|_| DomainError::Validation {
            field: "evidence end",
            reason: "does not fit this platform".into(),
        })?;
        if !content.is_char_boundary(start_usize) || !content.is_char_boundary(end_usize) {
            return Err(DomainError::Validation {
                field: "evidence range",
                reason: "must align with UTF-8 boundaries".into(),
            });
        }
        Ok(Self {
            artifact,
            start,
            end,
            label,
        })
    }
}

/// Scope of a knowledge record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeScope {
    Personal,
    Organization,
    SourceDerived,
}

/// Review state of derived knowledge or a trace edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unreviewed,
    Verified,
    Disputed,
    Stale,
    Rejected,
}

/// One evidence-backed assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub text: String,
    pub evidence: Vec<EvidenceReference>,
}

impl Claim {
    /// Creates a claim that must cite evidence.
    pub fn new(
        id: ClaimId,
        text: impl Into<String>,
        evidence: Vec<EvidenceReference>,
    ) -> Result<Self, DomainError> {
        let text = text.into();
        non_blank("claim text", &text)?;
        if evidence.is_empty() {
            return Err(DomainError::Validation {
                field: "claim evidence",
                reason: "at least one evidence reference is required".into(),
            });
        }
        Ok(Self { id, text, evidence })
    }
}

/// Reviewable collection of claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub title: String,
    pub scope: KnowledgeScope,
    pub status: VerificationStatus,
    pub claims: Vec<Claim>,
}

impl Finding {
    /// Creates an evidence-backed finding.
    pub fn new(
        id: FindingId,
        title: impl Into<String>,
        scope: KnowledgeScope,
        status: VerificationStatus,
        claims: Vec<Claim>,
    ) -> Result<Self, DomainError> {
        let title = title.into();
        non_blank("finding title", &title)?;
        if claims.is_empty() {
            return Err(DomainError::Validation {
                field: "finding claims",
                reason: "at least one claim is required".into(),
            });
        }
        Ok(Self {
            id,
            title,
            scope,
            status,
            claims,
        })
    }
}

/// Entity receiving feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum FeedbackTarget {
    Artifact(ArtifactId),
    Finding(FindingId),
    Claim(ClaimId),
    TraceEdge(TraceEdgeId),
    Relationship(RelationshipId),
}

/// Scoped correction or annotation that does not mutate its target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub id: FeedbackId,
    pub target: FeedbackTarget,
    pub scope: KnowledgeScope,
    pub text: String,
    pub status: VerificationStatus,
}

impl Feedback {
    /// Creates feedback in a personal or organization scope.
    pub fn new(
        id: FeedbackId,
        target: FeedbackTarget,
        scope: KnowledgeScope,
        text: impl Into<String>,
        status: VerificationStatus,
    ) -> Result<Self, DomainError> {
        if scope == KnowledgeScope::SourceDerived {
            return Err(DomainError::Validation {
                field: "feedback scope",
                reason: "source-derived is not an authorship scope".into(),
            });
        }
        let text = text.into();
        non_blank("feedback text", &text)?;
        if text.len() > 4096 {
            return Err(DomainError::Validation {
                field: "feedback text",
                reason: "must not exceed 4096 bytes".into(),
            });
        }
        Ok(Self {
            id,
            target,
            scope,
            text,
            status,
        })
    }
}

/// Provenance of a trace edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrigin {
    Deterministic,
    Extracted,
    Inferred,
    Manual,
    Imported,
}

/// Kind and canonical reference of a trace node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reference", rename_all = "snake_case")]
pub enum TraceNodeKind {
    Finding(FindingId),
    Evidence(String),
    Snapshot(SnapshotId),
    ProcessingStep(String),
}

/// Node in a lineage trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceNode {
    pub id: TraceNodeId,
    pub label: String,
    pub kind: TraceNodeKind,
}

impl TraceNode {
    pub fn new(
        id: TraceNodeId,
        label: impl Into<String>,
        kind: TraceNodeKind,
    ) -> Result<Self, DomainError> {
        let label = label.into();
        non_blank("trace node label", &label)?;
        Ok(Self { id, label, kind })
    }
}

/// Directed relationship in a lineage trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEdge {
    pub id: TraceEdgeId,
    pub from: TraceNodeId,
    pub to: TraceNodeId,
    pub origin: EdgeOrigin,
    pub status: VerificationStatus,
}

/// Valid contiguous path through a trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracePath {
    pub nodes: Vec<TraceNodeId>,
    pub edges: Vec<TraceEdgeId>,
}

/// Validated directed acyclic lineage graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub id: TraceId,
    nodes: BTreeMap<TraceNodeId, TraceNode>,
    edges: BTreeMap<TraceEdgeId, TraceEdge>,
}

impl Trace {
    pub fn new(id: TraceId) -> Self {
        Self {
            id,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
    pub fn nodes(&self) -> impl Iterator<Item = &TraceNode> {
        self.nodes.values()
    }
    pub fn edges(&self) -> impl Iterator<Item = &TraceEdge> {
        self.edges.values()
    }
    pub fn add_node(&mut self, node: TraceNode) -> Result<(), DomainError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DomainError::TraceInvariant(format!(
                "duplicate node {}",
                node.id
            )));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }
    pub fn add_edge(&mut self, edge: TraceEdge) -> Result<(), DomainError> {
        if edge.from == edge.to {
            return Err(DomainError::TraceInvariant(
                "self-edges are forbidden".into(),
            ));
        }
        if self.edges.contains_key(&edge.id) {
            return Err(DomainError::TraceInvariant(format!(
                "duplicate edge {}",
                edge.id
            )));
        }
        for endpoint in [&edge.from, &edge.to] {
            if !self.nodes.contains_key(endpoint) {
                return Err(DomainError::NotFound {
                    entity: "trace node",
                    id: endpoint.to_string(),
                });
            }
        }
        if self.reachable(&edge.to, &edge.from) {
            return Err(DomainError::TraceInvariant(
                "edge would introduce a cycle".into(),
            ));
        }
        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }
    fn reachable(&self, from: &TraceNodeId, to: &TraceNodeId) -> bool {
        let mut queue = VecDeque::from([from.clone()]);
        let mut seen = BTreeSet::new();
        while let Some(node) = queue.pop_front() {
            if &node == to {
                return true;
            }
            if seen.insert(node.clone()) {
                for edge in self.edges.values().filter(|edge| edge.from == node) {
                    queue.push_back(edge.to.clone());
                }
            }
        }
        false
    }
    pub fn path(&self, from: &TraceNodeId, to: &TraceNodeId) -> Result<TracePath, DomainError> {
        let mut queue = VecDeque::from([from.clone()]);
        let mut parent: BTreeMap<TraceNodeId, (TraceNodeId, TraceEdgeId)> = BTreeMap::new();
        let mut seen = BTreeSet::from([from.clone()]);
        while let Some(node) = queue.pop_front() {
            if &node == to {
                break;
            }
            for edge in self.edges.values().filter(|edge| edge.from == node) {
                if seen.insert(edge.to.clone()) {
                    parent.insert(edge.to.clone(), (node.clone(), edge.id.clone()));
                    queue.push_back(edge.to.clone());
                }
            }
        }
        if !seen.contains(to) {
            return Err(DomainError::NotFound {
                entity: "trace path",
                id: format!("{from}->{to}"),
            });
        }
        let mut nodes = vec![to.clone()];
        let mut edges = Vec::new();
        let mut cursor = to.clone();
        while &cursor != from {
            let (previous, edge) = parent
                .get(&cursor)
                .ok_or_else(|| DomainError::TraceInvariant("path parent missing".into()))?;
            edges.push(edge.clone());
            nodes.push(previous.clone());
            cursor = previous.clone();
        }
        nodes.reverse();
        edges.reverse();
        Ok(TracePath { nodes, edges })
    }
}

/// Saved dashboard/view definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedView {
    pub id: SavedViewId,
    pub name: String,
    pub scope: KnowledgeScope,
}

impl SavedView {
    pub fn new(
        id: SavedViewId,
        name: impl Into<String>,
        scope: KnowledgeScope,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        non_blank("saved view name", &name)?;
        Ok(Self { id, name, scope })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn feedback_cannot_use_source_derived_scope() {
        let result = Feedback::new(
            FeedbackId::deterministic("f"),
            FeedbackTarget::Finding(FindingId::deterministic("x")),
            KnowledgeScope::SourceDerived,
            "correction",
            VerificationStatus::Unreviewed,
        );
        assert!(matches!(
            result,
            Err(DomainError::Validation {
                field: "feedback scope",
                ..
            })
        ));
    }

    #[test]
    fn trace_rejects_cycles_and_finds_path() {
        let mut trace = Trace::new(TraceId::deterministic("trace"));
        let a = TraceNodeId::deterministic("a");
        let b = TraceNodeId::deterministic("b");
        let c = TraceNodeId::deterministic("c");
        for id in [&a, &b, &c] {
            trace
                .add_node(
                    TraceNode::new(
                        id.clone(),
                        id.to_string(),
                        TraceNodeKind::ProcessingStep("test".into()),
                    )
                    .expect("valid node"),
                )
                .expect("unique node");
        }
        trace
            .add_edge(TraceEdge {
                id: TraceEdgeId::deterministic("ab"),
                from: a.clone(),
                to: b.clone(),
                origin: EdgeOrigin::Manual,
                status: VerificationStatus::Unreviewed,
            })
            .expect("valid edge");
        trace
            .add_edge(TraceEdge {
                id: TraceEdgeId::deterministic("bc"),
                from: b.clone(),
                to: c.clone(),
                origin: EdgeOrigin::Deterministic,
                status: VerificationStatus::Verified,
            })
            .expect("valid edge");
        assert_eq!(
            trace.path(&a, &c).expect("path").nodes,
            vec![a.clone(), b, c.clone()]
        );
        assert!(trace
            .add_edge(TraceEdge {
                id: TraceEdgeId::deterministic("ca"),
                from: c,
                to: a,
                origin: EdgeOrigin::Imported,
                status: VerificationStatus::Unreviewed
            })
            .is_err());
    }

    proptest! {
        #[test]
        fn deterministic_ids_always_round_trip(seed in any::<Vec<u8>>()) {
            let id = ArtifactId::deterministic(seed);
            prop_assert_eq!(ArtifactId::parse(id.to_string()), Ok(id));
        }

        #[test]
        fn evidence_accepts_exact_valid_ascii_ranges(start in 0usize..32, width in 1usize..32) {
            let text = "x".repeat(start + width);
            let evidence = EvidenceReference::new(ArtifactReference { artifact_id: ArtifactId::deterministic("a") }, start as u64, (start + width) as u64, "range", &text);
            prop_assert!(evidence.is_ok());
        }
    }
}
