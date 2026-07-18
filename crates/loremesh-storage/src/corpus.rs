use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use loremesh_core::corpus::{CorpusManifest, ManifestArtifact, ManifestRelationship};
use loremesh_core::index::IndexDocument;
use loremesh_core::relationship::{
    CodeReference, ExternalProvenance, RelationType, Relationship, RelationshipEndpoint,
};
use loremesh_core::{
    Artifact, ArtifactId, CodeReferenceId, EdgeOrigin, Feedback, FeedbackTarget, SnapshotId,
    Source, SourceId, SourceSnapshot, VerificationStatus,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{db, io, ser, LocalRepository, StorageError, METADATA_DIR, OBJECTS_DIR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusImportLimits {
    pub max_manifest_bytes: u64,
    pub max_artifacts: usize,
    pub max_artifact_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for CorpusImportLimits {
    fn default() -> Self {
        Self {
            max_manifest_bytes: 2 * 1024 * 1024,
            max_artifacts: 10_000,
            max_artifact_bytes: 16 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
        }
    }
}

impl CorpusImportLimits {
    /// Bounded limits for explicitly requested, local scale-corpus imports.
    #[must_use]
    pub const fn large_local() -> Self {
        Self {
            max_manifest_bytes: 256 * 1024 * 1024,
            max_artifacts: 100_000,
            max_artifact_bytes: 64 * 1024 * 1024,
            max_total_bytes: 3 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CorpusImportResult {
    pub corpus_name: String,
    pub documents_discovered: u64,
    pub documents_imported: u64,
    pub snapshots_created: u64,
    pub unchanged_sources: u64,
    pub images: u64,
    pub issues: u64,
    pub code_references: u64,
    pub relationships: u64,
    pub external_relationships: u64,
    pub diagnostics: Vec<CorpusDiagnostic>,
}

impl LocalRepository {
    pub fn import_corpus_manifest(
        &mut self,
        manifest_path: &Path,
        limits: CorpusImportLimits,
    ) -> Result<CorpusImportResult, StorageError> {
        let (manifest, root) = load_manifest(manifest_path, limits)?;
        let mut result = empty_result(&manifest);
        let source_ids = unique_ids(
            manifest.sources.iter().map(|source| source.id.as_str()),
            "source",
            &mut result.diagnostics,
        );
        let artifact_ids = unique_ids(
            manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.id.as_str()),
            "artifact",
            &mut result.diagnostics,
        );
        let mut imported = BTreeMap::new();
        let mut total_bytes = 0_u64;
        for artifact in &manifest.artifacts {
            if !artifact_ids.contains(&artifact.id) {
                continue;
            }
            if !source_ids.contains(&artifact.source) {
                result.diagnostics.push(error(
                    "missing_source",
                    &artifact.id,
                    format!("unknown manifest source {}", artifact.source),
                ));
                continue;
            }
            match self.import_manifest_artifact(
                &root,
                &manifest.name,
                artifact,
                limits,
                &mut total_bytes,
            ) {
                Ok(import) => {
                    result.documents_imported += 1;
                    if import.inserted {
                        result.snapshots_created += 1;
                    } else {
                        result.unchanged_sources += 1;
                    }
                    if artifact.media_type.starts_with("image/") {
                        result.images += 1;
                    }
                    if artifact.document_type == "issue" {
                        result.issues += 1;
                    }
                    imported.insert(artifact.id.clone(), import.artifact.id);
                }
                Err(StorageError::Io { .. }) if !root.join(&artifact.path).exists() => {
                    result.diagnostics.push(error(
                        "missing_file",
                        &artifact.id,
                        format!("content path {} does not exist", artifact.path),
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        let code_ids = self.store_code_references(&manifest, &mut result)?;
        let analyses = manifest
            .external_analyses
            .iter()
            .map(|analysis| (analysis.id.as_str(), analysis))
            .collect::<BTreeMap<_, _>>();
        for relationship in &manifest.relationships {
            match translate_relationship(relationship, &imported, &code_ids, &analyses) {
                Ok(value) => {
                    let external = value.external_provenance.is_some();
                    self.store_relationship(&value)?;
                    result.relationships += 1;
                    if external {
                        result.external_relationships += 1;
                    }
                }
                Err(message) => result.diagnostics.push(error(
                    "broken_relationship",
                    &format!(
                        "{}:{}:{}",
                        relationship.source, relationship.relation, relationship.target
                    ),
                    message,
                )),
            }
        }
        let manifest_json = serde_json::to_string(&manifest)
            .map_err(|source| ser("serializing corpus manifest", source))?;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO corpus_imports (name, version, body) VALUES (?1, ?2, ?3)",
                params![manifest.name, manifest.version, manifest_json],
            )
            .map_err(|source| db("recording corpus import", source))?;
        Ok(result)
    }

    fn import_manifest_artifact(
        &mut self,
        root: &Path,
        corpus_name: &str,
        entry: &ManifestArtifact,
        limits: CorpusImportLimits,
        total_bytes: &mut u64,
    ) -> Result<crate::ImportResult, StorageError> {
        validate_manifest_identity("artifact identity", &entry.id)?;
        validate_relative_path(&entry.path)?;
        let candidate = root.join(&entry.path);
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|source| io("inspecting corpus artifact", source))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Validation(format!(
                "corpus artifact {} must be a regular non-symlink file",
                entry.id
            )));
        }
        let canonical = candidate
            .canonicalize()
            .map_err(|source| io("canonicalizing corpus artifact", source))?;
        if !canonical.starts_with(root) {
            return Err(StorageError::Validation(format!(
                "corpus artifact {} escapes its corpus root",
                entry.id
            )));
        }
        if metadata.len() > limits.max_artifact_bytes {
            return Err(StorageError::Validation(format!(
                "corpus artifact {} exceeds the per-artifact limit",
                entry.id
            )));
        }
        *total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| StorageError::Validation("corpus byte total overflowed".into()))?;
        if *total_bytes > limits.max_total_bytes {
            return Err(StorageError::Validation(
                "corpus exceeds the configured total-byte limit".into(),
            ));
        }
        let bytes = fs::read(&canonical).map_err(|source| io("reading corpus artifact", source))?;
        if entry.media_type.starts_with("text/") {
            std::str::from_utf8(&bytes).map_err(|_| {
                StorageError::Validation(format!(
                    "text corpus artifact {} must be valid UTF-8",
                    entry.id
                ))
            })?;
        }
        self.import_corpus_bytes(corpus_name, entry, &bytes)
    }

    fn import_corpus_bytes(
        &mut self,
        corpus_name: &str,
        entry: &ManifestArtifact,
        bytes: &[u8],
    ) -> Result<crate::ImportResult, StorageError> {
        let digest = hex::encode(Sha256::digest(bytes));
        let source = Source::local(
            SourceId::deterministic(format!("corpus:{corpus_name}:{}", entry.id)),
            format!("corpora/{corpus_name}/{}", entry.path),
        )?;
        let snapshot = SourceSnapshot::new(
            SnapshotId::deterministic(format!("{}:{digest}", source.id)),
            source.id.clone(),
            digest.clone(),
            bytes.len() as u64,
        )?;
        let artifact = Artifact::with_media_type(
            ArtifactId::deterministic(format!("{}:{digest}", source.id)),
            snapshot.id.clone(),
            entry.title.clone(),
            entry.media_type.clone(),
            bytes.len() as u64,
        )?;
        let object_path = self.root.join(METADATA_DIR).join(OBJECTS_DIR).join(&digest);
        if !object_path.exists() {
            fs::write(&object_path, bytes)
                .map_err(|source| io("writing corpus immutable object", source))?;
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| db("starting corpus artifact transaction", source))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO sources (id, location) VALUES (?1, ?2)",
                params![source.id.as_str(), source.location],
            )
            .map_err(|source| db("recording corpus source", source))?;
        let snapshot_inserted = transaction.execute(
            "INSERT OR IGNORE INTO snapshots (id, source_id, digest, byte_len) VALUES (?1, ?2, ?3, ?4)",
            params![snapshot.id.as_str(), snapshot.source_id.as_str(), snapshot.digest, snapshot.byte_len],
        ).map_err(|source| db("recording corpus snapshot", source))? == 1;
        transaction.execute(
            "INSERT OR IGNORE INTO artifacts (id, snapshot_id, name, media_type, byte_len) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![artifact.id.as_str(), artifact.snapshot_id.as_str(), artifact.name, artifact.media_type, artifact.byte_len],
        ).map_err(|source| db("recording corpus artifact", source))?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO current_snapshots (source_id, snapshot_id) VALUES (?1, ?2)",
                params![source.id.as_str(), snapshot.id.as_str()],
            )
            .map_err(|source| db("updating current corpus snapshot", source))?;
        transaction
            .commit()
            .map_err(|source| db("committing corpus artifact transaction", source))?;
        Ok(crate::ImportResult {
            source,
            snapshot,
            artifact,
            inserted: snapshot_inserted,
        })
    }

    fn store_code_references(
        &mut self,
        manifest: &CorpusManifest,
        result: &mut CorpusImportResult,
    ) -> Result<BTreeMap<String, CodeReferenceId>, StorageError> {
        let ids = unique_ids(
            manifest
                .code_references
                .iter()
                .map(|entry| entry.id.as_str()),
            "code_reference",
            &mut result.diagnostics,
        );
        let mut stored = BTreeMap::new();
        for entry in &manifest.code_references {
            if !ids.contains(&entry.id) {
                continue;
            }
            let line_range = match (entry.line_start, entry.line_end) {
                (Some(start), Some(end)) => Some((start, end)),
                (None, None) => None,
                _ => {
                    result.diagnostics.push(error(
                        "invalid_code_reference",
                        &entry.id,
                        "line_start and line_end must be provided together".into(),
                    ));
                    continue;
                }
            };
            let code = match CodeReference::new(
                CodeReferenceId::deterministic(format!("{}:{}", manifest.name, entry.id)),
                entry.repository.clone(),
                entry.revision.clone(),
                entry.path.clone(),
                entry.symbol.clone(),
                line_range,
            ) {
                Ok(code) => code,
                Err(error_value) => {
                    result.diagnostics.push(error(
                        "invalid_code_reference",
                        &entry.id,
                        error_value.to_string(),
                    ));
                    continue;
                }
            };
            let body = serde_json::to_string(&code)
                .map_err(|source| ser("serializing code reference", source))?;
            self.connection
                .execute(
                    "INSERT OR REPLACE INTO code_references (id, body) VALUES (?1, ?2)",
                    params![code.id.as_str(), body],
                )
                .map_err(|source| db("recording code reference", source))?;
            stored.insert(entry.id.clone(), code.id);
            result.code_references += 1;
        }
        Ok(stored)
    }

    fn store_relationship(&self, relationship: &Relationship) -> Result<(), StorageError> {
        let body = serde_json::to_string(relationship)
            .map_err(|source| ser("serializing relationship", source))?;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO relationships (id, body) VALUES (?1, ?2)",
                params![relationship.id.as_str(), body],
            )
            .map_err(|source| db("recording relationship", source))?;
        Ok(())
    }

    pub fn relationships(&self) -> Result<Vec<Relationship>, StorageError> {
        crate::load_json_rows(
            &self.connection,
            "SELECT body FROM relationships ORDER BY id",
            "loading relationships",
        )
    }

    pub fn save_feedback(&self, feedback: &Feedback) -> Result<(), StorageError> {
        let relationship_id = match &feedback.target {
            FeedbackTarget::Relationship(id) => Some(id.as_str()),
            _ => None,
        };
        if let Some(id) = relationship_id {
            let exists: bool = self
                .connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM relationships WHERE id = ?1)",
                    [id],
                    |row| row.get(0),
                )
                .map_err(|source| db("validating feedback relationship", source))?;
            if !exists {
                return Err(StorageError::Validation(format!(
                    "feedback relationship does not exist: {id}"
                )));
            }
        }
        let body = serde_json::to_string(feedback)
            .map_err(|source| ser("serializing feedback", source))?;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO feedback (id, relationship_id, body) VALUES (?1, ?2, ?3)",
                params![feedback.id.as_str(), relationship_id, body],
            )
            .map_err(|source| db("recording feedback", source))?;
        Ok(())
    }

    pub fn snapshots_for_source(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<SourceSnapshot>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, digest, byte_len FROM snapshots WHERE source_id = ?1 ORDER BY id")
            .map_err(|source| db("preparing source snapshot query", source))?;
        let rows = statement
            .query_map([source_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                ))
            })
            .map_err(|source| db("querying source snapshots", source))?;
        rows.map(|row| {
            let (id, digest, len) = row.map_err(|source| db("reading source snapshot", source))?;
            SourceSnapshot::new(SnapshotId::parse(id)?, source_id.clone(), digest, len)
                .map_err(StorageError::from)
        })
        .collect()
    }

    pub fn artifact_references_stale_snapshot(
        &self,
        artifact: &Artifact,
    ) -> Result<bool, StorageError> {
        let current: Option<String> = self.connection.query_row(
            "SELECT current_snapshots.snapshot_id FROM current_snapshots JOIN snapshots ON snapshots.source_id = current_snapshots.source_id WHERE snapshots.id = ?1",
            [artifact.snapshot_id.as_str()],
            |row| row.get(0),
        ).optional().map_err(|source| db("reading current snapshot", source))?;
        Ok(current.is_some_and(|id| id != artifact.snapshot_id.as_str()))
    }

    pub fn index_documents(&self) -> Result<Vec<IndexDocument>, StorageError> {
        let mut documents = Vec::new();
        for artifact in self.artifacts()? {
            if !artifact.media_type.starts_with("text/") {
                continue;
            }
            let content = self.artifact_content(&artifact)?;
            let source_id: String = self
                .connection
                .query_row(
                    "SELECT source_id FROM snapshots WHERE id = ?1",
                    [artifact.snapshot_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(|source| db("reading index source", source))?;
            let headings = content
                .lines()
                .filter_map(|line| {
                    line.trim_start()
                        .strip_prefix('#')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                })
                .collect();
            let document_type = if artifact.name.to_ascii_lowercase().contains("issue") {
                "issue"
            } else {
                "knowledge"
            };
            let document = IndexDocument {
                artifact_id: artifact.id,
                source_id: SourceId::parse(source_id)?,
                snapshot_id: artifact.snapshot_id,
                title: artifact.name,
                body: content,
                headings,
                document_type: document_type.into(),
                source_type: "corpus".into(),
                tags: Vec::new(),
            };
            document.validate()?;
            documents.push(document);
        }
        Ok(documents)
    }
}

fn unique_ids<'a>(
    values: impl Iterator<Item = &'a str>,
    kind: &str,
    diagnostics: &mut Vec<CorpusDiagnostic>,
) -> BTreeSet<String> {
    let mut once = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for value in values {
        if !once.insert(value.to_owned()) {
            duplicates.insert(value.to_owned());
        }
    }
    for duplicate in &duplicates {
        diagnostics.push(error(
            "duplicate_identity",
            duplicate,
            format!("duplicate {kind} identity"),
        ));
        once.remove(duplicate);
    }
    once
}

fn validate_manifest_identity(field: &'static str, value: &str) -> Result<(), StorageError> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(StorageError::Validation(format!(
            "{field} must be non-blank and at most 128 bytes"
        )));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), StorageError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || value.contains('\\')
    {
        return Err(StorageError::Validation(format!(
            "unsafe corpus-relative path: {value}"
        )));
    }
    Ok(())
}

fn translate_relationship(
    entry: &ManifestRelationship,
    artifacts: &BTreeMap<String, ArtifactId>,
    code: &BTreeMap<String, CodeReferenceId>,
    analyses: &BTreeMap<&str, &loremesh_core::corpus::ManifestExternalAnalysis>,
) -> Result<Relationship, String> {
    let source = endpoint(&entry.source, artifacts, code)?;
    let target = endpoint(&entry.target, artifacts, code)?;
    let relation =
        RelationType::parse(entry.relation.clone()).map_err(|error| error.to_string())?;
    let origin = parse_origin(&entry.origin)?;
    let status = parse_status(&entry.verification)?;
    let external_provenance = entry
        .external_analysis
        .as_deref()
        .map(|analysis_id| {
            let external_run = analyses
                .get(analysis_id)
                .ok_or_else(|| format!("unknown external analysis {analysis_id}"))?;
            Ok::<ExternalProvenance, String>(ExternalProvenance {
                provider: external_run.provider.clone(),
                provider_version: external_run.provider_version.clone(),
                run_id: external_run.run_id.clone(),
                configuration_digest: external_run.configuration_digest.clone(),
                observed_at: external_run.observed_at.clone(),
                external_id: entry.external_id.clone(),
            })
        })
        .transpose()?;
    Relationship::new(
        source,
        relation,
        target,
        origin,
        status,
        Vec::new(),
        external_provenance,
    )
    .map_err(|error| error.to_string())
}

fn endpoint(
    value: &str,
    artifacts: &BTreeMap<String, ArtifactId>,
    code: &BTreeMap<String, CodeReferenceId>,
) -> Result<RelationshipEndpoint, String> {
    if let Some(id) = value.strip_prefix("artifact:") {
        return artifacts
            .get(id)
            .cloned()
            .map(RelationshipEndpoint::Artifact)
            .ok_or_else(|| format!("unknown artifact endpoint {id}"));
    }
    if let Some(id) = value.strip_prefix("code:") {
        return code
            .get(id)
            .cloned()
            .map(RelationshipEndpoint::Code)
            .ok_or_else(|| format!("unknown code endpoint {id}"));
    }
    Err(format!("unsupported relationship endpoint {value}"))
}

fn parse_origin(value: &str) -> Result<EdgeOrigin, String> {
    match value {
        "manual" => Ok(EdgeOrigin::Manual),
        "deterministic" => Ok(EdgeOrigin::Deterministic),
        "imported" => Ok(EdgeOrigin::Imported),
        "extracted" => Ok(EdgeOrigin::Extracted),
        "inferred" => Ok(EdgeOrigin::Inferred),
        _ => Err(format!("unknown relationship origin {value}")),
    }
}

fn parse_status(value: &str) -> Result<VerificationStatus, String> {
    match value {
        "unreviewed" => Ok(VerificationStatus::Unreviewed),
        "verified" => Ok(VerificationStatus::Verified),
        "disputed" => Ok(VerificationStatus::Disputed),
        "stale" => Ok(VerificationStatus::Stale),
        "rejected" => Ok(VerificationStatus::Rejected),
        _ => Err(format!("unknown verification status {value}")),
    }
}

fn error(code: &str, subject: &str, message: String) -> CorpusDiagnostic {
    CorpusDiagnostic {
        severity: DiagnosticSeverity::Error,
        code: code.into(),
        subject: subject.into(),
        message,
    }
}

fn load_manifest(
    manifest_path: &Path,
    limits: CorpusImportLimits,
) -> Result<(CorpusManifest, std::path::PathBuf), StorageError> {
    let metadata = fs::symlink_metadata(manifest_path)
        .map_err(|source| io("inspecting corpus manifest", source))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(StorageError::Validation(
            "corpus manifest must be a regular non-symlink file".into(),
        ));
    }
    if metadata.len() > limits.max_manifest_bytes {
        return Err(StorageError::Validation(format!(
            "corpus manifest exceeds {} bytes; use --allow-large only for trusted local scale corpora",
            limits.max_manifest_bytes
        )));
    }
    let bytes = fs::read(manifest_path).map_err(|source| io("reading corpus manifest", source))?;
    let manifest: CorpusManifest =
        serde_json::from_slice(&bytes).map_err(|source| ser("parsing corpus manifest", source))?;
    if manifest.schema_version != 1 {
        return Err(StorageError::Validation(format!(
            "unsupported corpus schema version {}",
            manifest.schema_version
        )));
    }
    validate_manifest_identity("corpus name", &manifest.name)?;
    validate_manifest_identity("corpus version", &manifest.version)?;
    if manifest.artifacts.len() > limits.max_artifacts {
        return Err(StorageError::Validation(format!(
            "corpus has {} artifacts, exceeding limit {}",
            manifest.artifacts.len(),
            limits.max_artifacts
        )));
    }
    let root = manifest_path.parent().ok_or_else(|| {
        StorageError::Validation("corpus manifest must have a parent directory".into())
    })?;
    let root = root
        .canonicalize()
        .map_err(|source| io("canonicalizing corpus root", source))?;
    Ok((manifest, root))
}

fn empty_result(manifest: &CorpusManifest) -> CorpusImportResult {
    CorpusImportResult {
        corpus_name: manifest.name.clone(),
        documents_discovered: manifest.artifacts.len() as u64,
        documents_imported: 0,
        snapshots_created: 0,
        unchanged_sources: 0,
        images: 0,
        issues: 0,
        code_references: 0,
        relationships: 0,
        external_relationships: 0,
        diagnostics: Vec::new(),
    }
}
