//! Local filesystem and `SQLite` adapters for `LoreMesh`.
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use loremesh_core::{
    investigation::{EvidenceStatus, Investigation, InvestigationItem},
    relationship::CodeReference,
    Artifact, ArtifactId, Feedback, Finding, SnapshotId, Source, SourceId, SourceSnapshot, Trace,
    Workspace, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod corpus;
mod lexical;

pub use corpus::{CorpusDiagnostic, CorpusImportLimits, CorpusImportResult, DiagnosticSeverity};
pub use lexical::TantivyIndex;

const METADATA_DIR: &str = ".loremesh";
const DATABASE_FILE: &str = "loremesh.db";
const OBJECTS_DIR: &str = "objects";
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

/// Storage, configuration, and serialization failures.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("workspace configuration error: {0}")]
    Configuration(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("I/O error during {operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("SQLite storage error during {operation}: {source}")]
    Database {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("serialization error during {operation}: {source}")]
    Serialization {
        operation: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("domain invariant error: {0}")]
    Invariant(#[from] loremesh_core::DomainError),
}

/// Result of importing one immutable artifact.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub source: Source,
    pub snapshot: SourceSnapshot,
    pub artifact: Artifact,
    pub inserted: bool,
}

/// Counts safe to display without exposing content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub sources: u64,
    pub snapshots: u64,
    pub artifacts: u64,
    pub findings: u64,
}

/// Concrete local repository exercised by the foundation use cases.
pub struct LocalRepository {
    root: PathBuf,
    connection: Connection,
}

impl LocalRepository {
    /// Initializes a workspace or validates an existing compatible workspace.
    pub fn initialize(root: &Path) -> Result<Workspace, StorageError> {
        if root.exists() && !root.is_dir() {
            return Err(StorageError::Configuration(format!(
                "workspace path is not a directory: {}",
                root.display()
            )));
        }
        fs::create_dir_all(root).map_err(|source| io("creating workspace directory", source))?;
        let metadata = root.join(METADATA_DIR);
        fs::create_dir_all(metadata.join(OBJECTS_DIR))
            .map_err(|source| io("creating workspace metadata", source))?;
        let connection = Connection::open(metadata.join(DATABASE_FILE))
            .map_err(|source| db("opening workspace database", source))?;
        configure(&connection)?;
        migrate(&connection)?;
        let name = root
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("workspace");
        let id = WorkspaceId::deterministic(name.as_bytes());
        connection
            .execute(
                "INSERT OR IGNORE INTO workspace (singleton, id, name) VALUES (1, ?1, ?2)",
                params![id.as_str(), name],
            )
            .map_err(|source| db("recording workspace", source))?;
        Workspace::new(id, name, root.to_path_buf()).map_err(StorageError::from)
    }

    /// Opens an initialized workspace.
    pub fn open(root: &Path) -> Result<Self, StorageError> {
        let database = root.join(METADATA_DIR).join(DATABASE_FILE);
        if !database.is_file() {
            return Err(StorageError::Configuration(format!(
                "no LoreMesh workspace found at {}; run `loremesh workspace init <path>`",
                root.display()
            )));
        }
        let connection = Connection::open(database)
            .map_err(|source| db("opening workspace database", source))?;
        configure(&connection)?;
        migrate(&connection)?;
        validate_schema(&connection)?;
        Ok(Self {
            root: root.to_path_buf(),
            connection,
        })
    }

    /// Returns the workspace descriptor.
    pub fn workspace(&self) -> Result<Workspace, StorageError> {
        let (id, name): (String, String) = self
            .connection
            .query_row(
                "SELECT id, name FROM workspace WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|source| db("reading workspace", source))?;
        Workspace::new(WorkspaceId::parse(id)?, name, self.root.clone()).map_err(StorageError::from)
    }

    /// Imports one bounded UTF-8 Markdown file as immutable content.
    pub fn import_markdown(&mut self, input: &Path) -> Result<ImportResult, StorageError> {
        let metadata =
            fs::symlink_metadata(input).map_err(|source| io("inspecting import file", source))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Validation(
                "import path must be a regular non-symlink file".into(),
            ));
        }
        if metadata.len() > MAX_IMPORT_BYTES {
            return Err(StorageError::Validation(format!(
                "import exceeds the {MAX_IMPORT_BYTES}-byte foundation limit"
            )));
        }
        if input
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("md"))
        {
            return Err(StorageError::Validation(
                "foundation imports require a .md file".into(),
            ));
        }
        let bytes = fs::read(input).map_err(|source| io("reading import file", source))?;
        let _text = std::str::from_utf8(&bytes)
            .map_err(|_| StorageError::Validation("Markdown import must be valid UTF-8".into()))?;
        let name = input
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| StorageError::Validation("import filename must be valid UTF-8".into()))?
            .to_owned();
        let digest = hex::encode(Sha256::digest(&bytes));
        let source = Source::local(
            SourceId::deterministic(format!("local:{name}")),
            name.clone(),
        )?;
        let snapshot = SourceSnapshot::new(
            SnapshotId::deterministic(format!("{}:{digest}", source.id)),
            source.id.clone(),
            digest.clone(),
            bytes.len() as u64,
        )?;
        let artifact = Artifact::new(
            ArtifactId::deterministic(format!("{}:{digest}", source.id)),
            snapshot.id.clone(),
            name,
            bytes.len() as u64,
        )?;
        let object_path = self.root.join(METADATA_DIR).join(OBJECTS_DIR).join(&digest);
        if !object_path.exists() {
            fs::write(&object_path, &bytes)
                .map_err(|source| io("writing immutable object", source))?;
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| db("starting import transaction", source))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO sources (id, location) VALUES (?1, ?2)",
                params![source.id.as_str(), source.location],
            )
            .map_err(|source| db("recording source", source))?;
        transaction.execute("INSERT OR IGNORE INTO snapshots (id, source_id, digest, byte_len) VALUES (?1, ?2, ?3, ?4)", params![snapshot.id.as_str(), snapshot.source_id.as_str(), snapshot.digest, snapshot.byte_len]).map_err(|source| db("recording snapshot", source))?;
        let inserted = transaction.execute("INSERT OR IGNORE INTO artifacts (id, snapshot_id, name, media_type, byte_len) VALUES (?1, ?2, ?3, ?4, ?5)", params![artifact.id.as_str(), artifact.snapshot_id.as_str(), artifact.name, artifact.media_type, artifact.byte_len]).map_err(|source| db("recording artifact", source))? == 1;
        transaction
            .execute(
                "INSERT OR REPLACE INTO current_snapshots (source_id, snapshot_id) VALUES (?1, ?2)",
                params![source.id.as_str(), snapshot.id.as_str()],
            )
            .map_err(|source| db("recording current snapshot", source))?;
        transaction
            .commit()
            .map_err(|source| db("committing import", source))?;
        Ok(ImportResult {
            source,
            snapshot,
            artifact,
            inserted,
        })
    }

    /// Reads and digest-verifies an artifact's immutable content.
    pub fn artifact_content(&self, artifact: &Artifact) -> Result<String, StorageError> {
        let bytes = self.artifact_bytes(artifact)?;
        String::from_utf8(bytes).map_err(|_| {
            StorageError::Validation(format!("artifact {} is not valid UTF-8", artifact.id))
        })
    }

    /// Reads and digest-verifies an artifact's immutable bytes.
    pub fn artifact_bytes(&self, artifact: &Artifact) -> Result<Vec<u8>, StorageError> {
        let digest: String = self
            .connection
            .query_row(
                "SELECT digest FROM snapshots WHERE id = ?1",
                [artifact.snapshot_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| db("locating artifact object", source))?;
        let bytes = fs::read(self.root.join(METADATA_DIR).join(OBJECTS_DIR).join(&digest))
            .map_err(|source| io("reading immutable object", source))?;
        if hex::encode(Sha256::digest(&bytes)) != digest {
            return Err(StorageError::Validation(format!(
                "immutable object digest mismatch for artifact {}",
                artifact.id
            )));
        }
        Ok(bytes)
    }

    /// Lists artifacts in stable identifier order.
    pub fn artifacts(&self) -> Result<Vec<Artifact>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, snapshot_id, name, media_type, byte_len FROM artifacts ORDER BY id",
            )
            .map_err(|source| db("preparing artifact query", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, u64>(4)?,
                ))
            })
            .map_err(|source| db("querying artifacts", source))?;
        rows.map(|row| {
            let (id, snapshot, name, media_type, len) =
                row.map_err(|source| db("reading artifact row", source))?;
            Artifact::with_media_type(
                ArtifactId::parse(id)?,
                SnapshotId::parse(snapshot)?,
                name,
                media_type,
                len,
            )
            .map_err(StorageError::from)
        })
        .collect()
    }

    /// Stores a validated finding and trace as derived JSON in `SQLite`.
    pub fn save_investigation(
        &mut self,
        finding: &Finding,
        trace: &Trace,
    ) -> Result<(), StorageError> {
        let finding_json =
            serde_json::to_string(finding).map_err(|source| ser("serializing finding", source))?;
        let trace_json =
            serde_json::to_string(trace).map_err(|source| ser("serializing trace", source))?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| db("starting investigation transaction", source))?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO findings (id, body) VALUES (?1, ?2)",
                params![finding.id.as_str(), finding_json],
            )
            .map_err(|source| db("recording finding", source))?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO traces (id, finding_id, body) VALUES (?1, ?2, ?3)",
                params![trace.id.as_str(), finding.id.as_str(), trace_json],
            )
            .map_err(|source| db("recording trace", source))?;
        transaction
            .commit()
            .map_err(|source| db("committing investigation", source))
    }

    /// Lists findings in stable identifier order.
    pub fn findings(&self) -> Result<Vec<Finding>, StorageError> {
        load_json_rows(
            &self.connection,
            "SELECT body FROM findings ORDER BY id",
            "loading findings",
        )
    }

    /// Loads the trace associated with a finding.
    pub fn trace_for(&self, finding: &Finding) -> Result<Trace, StorageError> {
        let body: Option<String> = self
            .connection
            .query_row(
                "SELECT body FROM traces WHERE finding_id = ?1",
                [finding.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| db("loading trace", source))?;
        let body = body.ok_or_else(|| {
            StorageError::Validation(format!("no trace found for finding {}", finding.id))
        })?;
        serde_json::from_str(&body).map_err(|source| ser("deserializing trace", source))
    }

    /// Stores an investigation after validating every canonical reference.
    pub fn save_investigation_record(
        &self,
        investigation: &Investigation,
    ) -> Result<(), StorageError> {
        for item in &investigation.items {
            self.validate_investigation_item(item)?;
        }
        let body = serde_json::to_string(investigation)
            .map_err(|source| ser("serializing investigation", source))?;
        let scope = match investigation.scope {
            loremesh_core::investigation::InvestigationScope::Personal => "personal",
            loremesh_core::investigation::InvestigationScope::Organization => "organization",
        };
        self.connection
            .execute(
                "INSERT OR REPLACE INTO investigations (id, scope, body) VALUES (?1, ?2, ?3)",
                params![investigation.id.as_str(), scope, body],
            )
            .map_err(|source| db("recording investigation", source))?;
        Ok(())
    }

    /// Lists investigations in stable identifier order.
    pub fn investigations(&self) -> Result<Vec<Investigation>, StorageError> {
        load_json_rows(
            &self.connection,
            "SELECT body FROM investigations ORDER BY id",
            "loading investigations",
        )
    }

    /// Loads one investigation by stable ID.
    pub fn investigation(
        &self,
        id: &loremesh_core::InvestigationId,
    ) -> Result<Investigation, StorageError> {
        let body: Option<String> = self
            .connection
            .query_row(
                "SELECT body FROM investigations WHERE id = ?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| db("loading investigation", source))?;
        serde_json::from_str(&body.ok_or_else(|| {
            StorageError::Validation(format!("investigation does not exist: {id}"))
        })?)
        .map_err(|source| ser("deserializing investigation", source))
    }

    fn validate_investigation_item(&self, item: &InvestigationItem) -> Result<(), StorageError> {
        let valid = match item {
            InvestigationItem::Artifact(id) => self.row_exists("artifacts", id.as_str())?,
            InvestigationItem::Finding(id) => self.row_exists("findings", id.as_str())?,
            InvestigationItem::Claim(id) => self
                .findings()?
                .iter()
                .any(|finding| finding.claims.iter().any(|claim| claim.id == *id)),
            InvestigationItem::Evidence(evidence) => {
                self.row_exists("artifacts", evidence.artifact.artifact_id.as_str())?
            }
            InvestigationItem::Relationship(id) => self.row_exists("relationships", id.as_str())?,
            InvestigationItem::Trace(id) => self.row_exists("traces", id.as_str())?,
            InvestigationItem::CodeReference(id) => {
                self.row_exists("code_references", id.as_str())?
            }
        };
        if valid {
            Ok(())
        } else {
            Err(StorageError::Validation(format!(
                "investigation references unknown canonical object: {item:?}"
            )))
        }
    }

    fn row_exists(&self, table: &str, id: &str) -> Result<bool, StorageError> {
        let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)");
        self.connection
            .query_row(&sql, [id], |row| row.get(0))
            .map_err(|source| db("validating investigation reference", source))
    }

    /// Resolves one artifact by canonical ID.
    pub fn artifact(&self, id: &ArtifactId) -> Result<Option<Artifact>, StorageError> {
        Ok(self
            .artifacts()?
            .into_iter()
            .find(|artifact| artifact.id == *id))
    }

    /// Resolves the source and immutable snapshot for an artifact.
    pub fn artifact_lineage(
        &self,
        artifact: &Artifact,
    ) -> Result<(SourceSnapshot, Source), StorageError> {
        let (source_id, digest, byte_len): (String, String, u64) = self
            .connection
            .query_row(
                "SELECT source_id, digest, byte_len FROM snapshots WHERE id = ?1",
                [artifact.snapshot_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|source| db("loading artifact snapshot", source))?;
        let source_id = SourceId::parse(source_id)?;
        let location: String = self
            .connection
            .query_row(
                "SELECT location FROM sources WHERE id = ?1",
                [source_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| db("loading artifact source", source))?;
        Ok((
            SourceSnapshot::new(
                artifact.snapshot_id.clone(),
                source_id.clone(),
                digest,
                byte_len,
            )?,
            Source::local(source_id, location)?,
        ))
    }

    /// Classifies immutable evidence relative to the source's current snapshot.
    pub fn evidence_status(
        &self,
        evidence: &loremesh_core::EvidenceReference,
    ) -> Result<EvidenceStatus, StorageError> {
        let Some(artifact) = self.artifact(&evidence.artifact.artifact_id)? else {
            return Ok(EvidenceStatus::Missing);
        };
        let current: Option<String> = self.connection.query_row(
            "SELECT current_snapshots.snapshot_id FROM current_snapshots JOIN snapshots ON snapshots.source_id = current_snapshots.source_id WHERE snapshots.id = ?1",
            [artifact.snapshot_id.as_str()],
            |row| row.get(0),
        ).optional().map_err(|source| db("classifying evidence snapshot", source))?;
        Ok(match current.as_deref() {
            Some(id) if id == artifact.snapshot_id.as_str() => EvidenceStatus::Current,
            Some(_) => EvidenceStatus::Historical,
            None => EvidenceStatus::Missing,
        })
    }

    /// Lists persisted feedback in stable identifier order.
    pub fn feedback(&self) -> Result<Vec<Feedback>, StorageError> {
        load_json_rows(
            &self.connection,
            "SELECT body FROM feedback ORDER BY id",
            "loading feedback",
        )
    }

    /// Lists canonical code references in stable identifier order.
    pub fn code_references(&self) -> Result<Vec<CodeReference>, StorageError> {
        load_json_rows(
            &self.connection,
            "SELECT body FROM code_references ORDER BY id",
            "loading code references",
        )
    }

    /// Returns workspace entity counts.
    pub fn summary(&self) -> Result<WorkspaceSummary, StorageError> {
        Ok(WorkspaceSummary {
            sources: count(&self.connection, "sources")?,
            snapshots: count(&self.connection, "snapshots")?,
            artifacts: count(&self.connection, "artifacts")?,
            findings: count(&self.connection, "findings")?,
        })
    }

    /// Validates database and every referenced object digest.
    pub fn doctor(&self) -> Result<(), StorageError> {
        let integrity: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|source| db("checking database integrity", source))?;
        if integrity != "ok" {
            return Err(StorageError::Validation(format!(
                "database integrity check failed: {integrity}"
            )));
        }
        for artifact in self.artifacts()? {
            let _content = self.artifact_content(&artifact)?;
        }
        Ok(())
    }
}

fn configure(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(|source| db("configuring database", source))
}
fn migrate(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("BEGIN; CREATE TABLE IF NOT EXISTS schema_info (version INTEGER NOT NULL); INSERT INTO schema_info(version) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_info); CREATE TABLE IF NOT EXISTS workspace (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), id TEXT NOT NULL UNIQUE, name TEXT NOT NULL); CREATE TABLE IF NOT EXISTS sources (id TEXT PRIMARY KEY, location TEXT NOT NULL UNIQUE); CREATE TABLE IF NOT EXISTS snapshots (id TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES sources(id), digest TEXT NOT NULL, byte_len INTEGER NOT NULL, UNIQUE(source_id, digest)); CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, snapshot_id TEXT NOT NULL REFERENCES snapshots(id), name TEXT NOT NULL, media_type TEXT NOT NULL, byte_len INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS findings (id TEXT PRIMARY KEY, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS traces (id TEXT PRIMARY KEY, finding_id TEXT NOT NULL UNIQUE REFERENCES findings(id), body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS current_snapshots (source_id TEXT PRIMARY KEY REFERENCES sources(id), snapshot_id TEXT NOT NULL REFERENCES snapshots(id)); CREATE TABLE IF NOT EXISTS relationships (id TEXT PRIMARY KEY, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS feedback (id TEXT PRIMARY KEY, relationship_id TEXT, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS code_references (id TEXT PRIMARY KEY, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS corpus_imports (name TEXT NOT NULL, version TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(name, version)); CREATE TABLE IF NOT EXISTS investigations (id TEXT PRIMARY KEY, scope TEXT NOT NULL CHECK(scope IN ('personal', 'organization')), body TEXT NOT NULL); UPDATE schema_info SET version = 3; COMMIT;").map_err(|source| db("creating schema", source))
}
fn validate_schema(connection: &Connection) -> Result<(), StorageError> {
    let version: i64 = connection
        .query_row("SELECT version FROM schema_info LIMIT 1", [], |row| {
            row.get(0)
        })
        .map_err(|source| db("reading schema version", source))?;
    if version == 3 {
        Ok(())
    } else {
        Err(StorageError::Configuration(format!(
            "unsupported workspace schema version {version}"
        )))
    }
}
fn count(connection: &Connection, table: &str) -> Result<u64, StorageError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|source| db("counting workspace entities", source))
}
fn load_json_rows<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    operation: &'static str,
) -> Result<Vec<T>, StorageError> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|source| db(operation, source))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| db(operation, source))?;
    rows.map(|row| {
        let body = row.map_err(|source| db(operation, source))?;
        serde_json::from_str(&body).map_err(|source| ser(operation, source))
    })
    .collect()
}
fn io(operation: &'static str, source: std::io::Error) -> StorageError {
    StorageError::Io { operation, source }
}
fn db(operation: &'static str, source: rusqlite::Error) -> StorageError {
    StorageError::Database { operation, source }
}
fn ser(operation: &'static str, source: serde_json::Error) -> StorageError {
    StorageError::Serialization { operation, source }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loremesh_core::investigation::{InvestigationScope, InvestigationStatus};

    #[test]
    fn initialization_and_import_are_idempotent() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        LocalRepository::initialize(temporary.path()).expect("initialize");
        LocalRepository::initialize(temporary.path()).expect("initialize again");
        let input = temporary.path().join("sample.md");
        fs::write(&input, "# Sample\n\nEvidence.\n").expect("fixture");
        let mut repository = LocalRepository::open(temporary.path()).expect("open");
        let first = repository.import_markdown(&input).expect("first import");
        let second = repository.import_markdown(&input).expect("second import");
        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(first.artifact.id, second.artifact.id);
        assert_eq!(repository.summary().expect("summary").artifacts, 1);
        repository.doctor().expect("healthy workspace");
    }

    #[test]
    fn investigation_reloads_with_canonical_references_and_survives_index_absence() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        LocalRepository::initialize(temporary.path()).expect("initialize");
        let input = temporary.path().join("sample.md");
        fs::write(&input, "# Sample\n\nEvidence.\n").expect("fixture");
        let mut repository = LocalRepository::open(temporary.path()).expect("open");
        let imported = repository.import_markdown(&input).expect("import");
        let mut investigation = Investigation::new(
            loremesh_core::InvestigationId::deterministic("sample investigation"),
            "Sample investigation",
            "",
            InvestigationScope::Personal,
        )
        .expect("investigation");
        investigation.add_item(InvestigationItem::Artifact(imported.artifact.id.clone()));
        investigation
            .add_note("Review the source lineage.")
            .expect("note");
        investigation
            .transition_to(InvestigationStatus::InReview)
            .expect("transition");
        repository
            .save_investigation_record(&investigation)
            .expect("save investigation");
        drop(repository);

        let repository = LocalRepository::open(temporary.path()).expect("reopen");
        let loaded = repository
            .investigation(&investigation.id)
            .expect("reload investigation");
        assert_eq!(loaded, investigation);
        assert_eq!(loaded.scope, InvestigationScope::Personal);
        assert!(loaded
            .items
            .contains(&InvestigationItem::Artifact(imported.artifact.id)));
        assert!(!temporary
            .path()
            .join(".loremesh/indexes/knowledge")
            .exists());
    }

    #[test]
    fn unknown_investigation_reference_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        LocalRepository::initialize(temporary.path()).expect("initialize");
        let repository = LocalRepository::open(temporary.path()).expect("open");
        let mut investigation = Investigation::new(
            loremesh_core::InvestigationId::deterministic("invalid"),
            "Invalid reference",
            "",
            InvestigationScope::Personal,
        )
        .expect("investigation");
        investigation.add_item(InvestigationItem::Artifact(ArtifactId::deterministic(
            "missing",
        )));
        assert!(repository
            .save_investigation_record(&investigation)
            .is_err());
    }

    #[test]
    fn newer_snapshot_marks_old_evidence_historical_without_redirecting_it() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        LocalRepository::initialize(temporary.path()).expect("initialize");
        let input = temporary.path().join("sample.md");
        fs::write(&input, "Old evidence").expect("old fixture");
        let mut repository = LocalRepository::open(temporary.path()).expect("open");
        let old = repository.import_markdown(&input).expect("old import");
        let evidence = loremesh_core::EvidenceReference::new(
            loremesh_core::ArtifactReference {
                artifact_id: old.artifact.id.clone(),
            },
            0,
            3,
            "old",
            "Old evidence",
        )
        .expect("evidence");
        assert_eq!(
            repository
                .evidence_status(&evidence)
                .expect("current status"),
            EvidenceStatus::Current
        );
        fs::write(&input, "New evidence").expect("new fixture");
        let new = repository.import_markdown(&input).expect("new import");
        assert_ne!(old.artifact.snapshot_id, new.artifact.snapshot_id);
        assert_eq!(evidence.artifact.artifact_id, old.artifact.id);
        assert_eq!(
            repository
                .evidence_status(&evidence)
                .expect("historical status"),
            EvidenceStatus::Historical
        );
    }
}
