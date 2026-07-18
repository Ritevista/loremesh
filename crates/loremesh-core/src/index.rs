//! Replaceable lexical-index boundary models.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ArtifactId, DomainError, SnapshotId, SourceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDocument {
    pub artifact_id: ArtifactId,
    pub source_id: SourceId,
    pub snapshot_id: SnapshotId,
    pub title: String,
    pub body: String,
    pub headings: Vec<String>,
    pub document_type: String,
    pub source_type: String,
    pub tags: Vec<String>,
}

impl IndexDocument {
    pub fn validate(&self) -> Result<(), DomainError> {
        for (field, value) in [
            ("index title", self.title.as_str()),
            ("index document type", self.document_type.as_str()),
            ("index source type", self.source_type.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(DomainError::Validation {
                    field,
                    reason: "must not be blank".into(),
                });
            }
        }
        if self.body.len() > 16 * 1024 * 1024 {
            return Err(DomainError::Validation {
                field: "index body",
                reason: "must not exceed 16 MiB per document".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub limit: usize,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>, limit: usize) -> Result<Self, DomainError> {
        let text = text.into();
        if text.trim().is_empty() || text.len() > 1024 {
            return Err(DomainError::Validation {
                field: "search query",
                reason: "must be non-blank and at most 1024 bytes".into(),
            });
        }
        if !(1..=1000).contains(&limit) {
            return Err(DomainError::Validation {
                field: "search limit",
                reason: "must be between 1 and 1000".into(),
            });
        }
        Ok(Self { text, limit })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub artifact_id: ArtifactId,
    pub source_id: SourceId,
    pub snapshot_id: SnapshotId,
    pub title: String,
    pub score: f32,
    pub excerpt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    NotBuilt,
    Building,
    Ready,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStatus {
    pub state: IndexState,
    pub schema_version: u32,
    pub documents: u64,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexBuildResult {
    pub indexed: u64,
}

#[derive(Debug, Error)]
pub enum LexicalIndexError {
    #[error("index validation error: {0}")]
    Validation(String),
    #[error("index is not built")]
    NotBuilt,
    #[error("index I/O error during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
    },
    #[error("index engine error during {operation}: {message}")]
    Engine {
        operation: &'static str,
        message: String,
    },
}

pub trait LexicalIndex {
    fn rebuild(
        &mut self,
        documents: Vec<IndexDocument>,
    ) -> Result<IndexBuildResult, LexicalIndexError>;
    fn remove(&mut self, artifact_id: &ArtifactId) -> Result<(), LexicalIndexError>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, LexicalIndexError>;
    fn status(&self) -> Result<IndexStatus, LexicalIndexError>;
    fn drop_index(&mut self) -> Result<(), LexicalIndexError>;
}
