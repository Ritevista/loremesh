use std::fs;
use std::path::{Path, PathBuf};

use loremesh_core::index::{
    IndexBuildResult, IndexDocument, IndexState, IndexStatus, LexicalIndex, LexicalIndexError,
    SearchHit, SearchQuery,
};
use loremesh_core::{ArtifactId, SnapshotId, SourceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, TantivyDocument, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

const INDEX_SCHEMA_VERSION: u32 = 1;
const INDEX_METADATA_FILE: &str = "loremesh-index.json";
const WRITER_MEMORY_BYTES: usize = 50_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeIndexMetadata {
    schema_version: u32,
    corpus_signature: String,
    documents: u64,
}

pub struct TantivyIndex {
    path: PathBuf,
}

impl TantivyIndex {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn knowledge_for_workspace(root: &Path) -> Self {
        Self::new(root.join(".loremesh").join("indexes").join("knowledge"))
    }

    pub fn metadata_path(&self) -> PathBuf {
        self.path.join(INDEX_METADATA_FILE)
    }

    fn schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("artifact_id", STRING | STORED);
        builder.add_text_field("source_id", STRING | STORED);
        builder.add_text_field("snapshot_id", STRING | STORED);
        builder.add_text_field("title", TEXT | STORED);
        builder.add_text_field("body", TEXT);
        builder.add_text_field("headings", TEXT);
        builder.add_text_field("document_type", STRING);
        builder.add_text_field("source_type", STRING);
        builder.add_text_field("tags", TEXT);
        builder.add_u64_field("schema_version", STORED);
        builder.build()
    }

    fn open(&self) -> Result<Index, LexicalIndexError> {
        if !self.path.join("meta.json").is_file() {
            return Err(LexicalIndexError::NotBuilt);
        }
        Index::open_in_dir(&self.path).map_err(|error| engine("opening index", error))
    }

    fn writer(index: &Index) -> Result<IndexWriter, LexicalIndexError> {
        index
            .writer(WRITER_MEMORY_BYTES)
            .map_err(|error| engine("creating index writer", error))
    }

    fn reader(index: &Index) -> Result<IndexReader, LexicalIndexError> {
        index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|error| engine("creating index reader", error))
    }

    fn workspace_root(&self) -> Option<PathBuf> {
        self.path
            .ancestors()
            .nth(3)
            .map(Path::to_path_buf)
    }

    fn metadata(&self) -> Result<KnowledgeIndexMetadata, LexicalIndexError> {
        let bytes =
            fs::read(self.metadata_path()).map_err(|error| io("reading index metadata", &error))?;
        serde_json::from_slice(&bytes).map_err(|error| {
            LexicalIndexError::Validation(format!("invalid index metadata: {error}"))
        })
    }

    fn write_metadata(&self, metadata: &KnowledgeIndexMetadata) -> Result<(), LexicalIndexError> {
        let bytes = serde_json::to_vec_pretty(metadata).map_err(|error| {
            LexicalIndexError::Validation(format!("serializing index metadata failed: {error}"))
        })?;
        fs::write(self.metadata_path(), bytes).map_err(|error| io("writing index metadata", &error))
    }

    fn corpus_signature(documents: &[IndexDocument]) -> String {
        let mut documents = documents.to_vec();
        documents.sort_by(|left, right| left.artifact_id.as_str().cmp(right.artifact_id.as_str()));
        let mut hasher = Sha256::new();
        for document in documents {
            hasher.update(document.artifact_id.as_str());
            hasher.update(document.source_id.as_str());
            hasher.update(document.snapshot_id.as_str());
            hasher.update(document.title.as_bytes());
            hasher.update(document.body.as_bytes());
            for heading in document.headings {
                hasher.update(heading.as_bytes());
            }
            hasher.update(document.document_type.as_bytes());
            hasher.update(document.source_type.as_bytes());
            for tag in document.tags {
                hasher.update(tag.as_bytes());
            }
        }
        hex::encode(hasher.finalize())
    }
}

impl LexicalIndex for TantivyIndex {
    fn rebuild(
        &mut self,
        documents: Vec<IndexDocument>,
    ) -> Result<IndexBuildResult, LexicalIndexError> {
        for document in &documents {
            document
                .validate()
                .map_err(|error| LexicalIndexError::Validation(error.to_string()))?;
        }
        if self.path.exists() {
            fs::remove_dir_all(&self.path)
                .map_err(|error| io("removing previous index", &error))?;
        }
        fs::create_dir_all(&self.path).map_err(|error| io("creating index directory", &error))?;
        let schema = Self::schema();
        let index = Index::create_in_dir(&self.path, schema.clone())
            .map_err(|error| engine("creating index", error))?;
        let artifact_id = schema
            .get_field("artifact_id")
            .map_err(|error| engine("reading artifact field", error))?;
        let source_id = schema
            .get_field("source_id")
            .map_err(|error| engine("reading source field", error))?;
        let snapshot_id = schema
            .get_field("snapshot_id")
            .map_err(|error| engine("reading snapshot field", error))?;
        let title = schema
            .get_field("title")
            .map_err(|error| engine("reading title field", error))?;
        let body = schema
            .get_field("body")
            .map_err(|error| engine("reading body field", error))?;
        let headings = schema
            .get_field("headings")
            .map_err(|error| engine("reading headings field", error))?;
        let document_type = schema
            .get_field("document_type")
            .map_err(|error| engine("reading document type field", error))?;
        let source_type = schema
            .get_field("source_type")
            .map_err(|error| engine("reading source type field", error))?;
        let tags = schema
            .get_field("tags")
            .map_err(|error| engine("reading tags field", error))?;
        let schema_version = schema
            .get_field("schema_version")
            .map_err(|error| engine("reading schema version field", error))?;
        let mut writer = Self::writer(&index)?;
        let indexed = documents.len() as u64;
        let corpus_signature = Self::corpus_signature(&documents);
        for document in documents {
            writer
                .add_document(doc!(
                    artifact_id => document.artifact_id.as_str(),
                    source_id => document.source_id.as_str(),
                    snapshot_id => document.snapshot_id.as_str(),
                    title => document.title,
                    body => document.body,
                    headings => document.headings.join("\n"),
                    document_type => document.document_type,
                    source_type => document.source_type,
                    tags => document.tags.join(" "),
                    schema_version => u64::from(INDEX_SCHEMA_VERSION),
                ))
                .map_err(|error| engine("adding index document", error))?;
        }
        writer
            .commit()
            .map_err(|error| engine("committing index", error))?;
        writer
            .wait_merging_threads()
            .map_err(|error| engine("merging index", error))?;
        self.write_metadata(&KnowledgeIndexMetadata {
            schema_version: INDEX_SCHEMA_VERSION,
            corpus_signature,
            documents: indexed,
        })?;
        Ok(IndexBuildResult { indexed })
    }

    fn remove(&mut self, artifact: &ArtifactId) -> Result<(), LexicalIndexError> {
        let index = self.open()?;
        let field = index
            .schema()
            .get_field("artifact_id")
            .map_err(|error| engine("reading artifact field", error))?;
        let mut writer = Self::writer(&index)?;
        writer.delete_term(Term::from_field_text(field, artifact.as_str()));
        writer
            .commit()
            .map_err(|error| engine("committing removal", error))?;
        Ok(())
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, LexicalIndexError> {
        let index = self.open()?;
        let schema = index.schema();
        let title = schema
            .get_field("title")
            .map_err(|error| engine("reading title field", error))?;
        let body = schema
            .get_field("body")
            .map_err(|error| engine("reading body field", error))?;
        let headings = schema
            .get_field("headings")
            .map_err(|error| engine("reading headings field", error))?;
        let artifact_id = schema
            .get_field("artifact_id")
            .map_err(|error| engine("reading artifact field", error))?;
        let source_id = schema
            .get_field("source_id")
            .map_err(|error| engine("reading source field", error))?;
        let snapshot_id = schema
            .get_field("snapshot_id")
            .map_err(|error| engine("reading snapshot field", error))?;
        let reader = Self::reader(&index)?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(&index, vec![title, body, headings]);
        let parsed = parser.parse_query(&query.text).map_err(|error| {
            LexicalIndexError::Validation(format!("invalid search query: {error}"))
        })?;
        let matches = searcher
            .search(&parsed, &TopDocs::with_limit(query.limit).order_by_score())
            .map_err(|error| engine("searching index", error))?;
        matches
            .into_iter()
            .map(|(score, address)| {
                let document: TantivyDocument = searcher
                    .doc(address)
                    .map_err(|error| engine("reading search result", error))?;
                let value = |field| {
                    document
                        .get_first(field)
                        .and_then(|stored| stored.as_str())
                        .ok_or_else(|| LexicalIndexError::Engine {
                            operation: "reading search result",
                            message: "stored field is missing".into(),
                        })
                };
                let title_value = value(title)?.to_owned();
                Ok(SearchHit {
                    artifact_id: ArtifactId::parse(value(artifact_id)?.to_owned())
                        .map_err(|error| LexicalIndexError::Validation(error.to_string()))?,
                    source_id: SourceId::parse(value(source_id)?.to_owned())
                        .map_err(|error| LexicalIndexError::Validation(error.to_string()))?,
                    snapshot_id: SnapshotId::parse(value(snapshot_id)?.to_owned())
                        .map_err(|error| LexicalIndexError::Validation(error.to_string()))?,
                    title: title_value.clone(),
                    score,
                    excerpt: title_value.chars().take(240).collect(),
                })
            })
            .collect()
    }

    fn status(&self) -> Result<IndexStatus, LexicalIndexError> {
        if !self.path.join("meta.json").is_file() {
            return Ok(IndexStatus {
                state: IndexState::NotBuilt,
                schema_version: INDEX_SCHEMA_VERSION,
                documents: 0,
                failure: None,
            });
        }
        let index = self.open()?;
        let reader = Self::reader(&index)?;
        let metadata = self.metadata().map_err(|error| match error {
            LexicalIndexError::Io { .. } | LexicalIndexError::Engine { .. } => error,
            other => other,
        })?;
        if metadata.schema_version != INDEX_SCHEMA_VERSION {
            return Ok(IndexStatus {
                state: IndexState::Failed,
                schema_version: metadata.schema_version,
                documents: reader.searcher().num_docs(),
                failure: Some("knowledge index metadata schema version mismatch".into()),
            });
        }
        let workspace_root = self.workspace_root().ok_or_else(|| {
            LexicalIndexError::Validation(
                "knowledge index path does not belong to a workspace".into(),
            )
        })?;
        let repository = crate::LocalRepository::open(&workspace_root).map_err(|error| {
            LexicalIndexError::Validation(format!(
                "opening workspace for index status failed: {error}"
            ))
        })?;
        let current_signature = repository.knowledge_index_signature().map_err(|error| {
            LexicalIndexError::Validation(format!("reading workspace signature failed: {error}"))
        })?;
        if metadata.corpus_signature != current_signature {
            return Ok(IndexStatus {
                state: IndexState::Stale,
                schema_version: INDEX_SCHEMA_VERSION,
                documents: reader.searcher().num_docs(),
                failure: Some(
                    "knowledge corpus has changed since the index was built; rebuild the knowledge index".into(),
                ),
            });
        }
        Ok(IndexStatus {
            state: IndexState::Ready,
            schema_version: INDEX_SCHEMA_VERSION,
            documents: reader.searcher().num_docs(),
            failure: None,
        })
    }

    fn drop_index(&mut self) -> Result<(), LexicalIndexError> {
        if self.path.exists() {
            fs::remove_dir_all(&self.path).map_err(|error| io("dropping index", &error))?;
        }
        Ok(())
    }
}

fn io(operation: &'static str, error: &std::io::Error) -> LexicalIndexError {
    LexicalIndexError::Io {
        operation,
        message: error.to_string(),
    }
}

fn engine(operation: &'static str, error: impl std::fmt::Display) -> LexicalIndexError {
    LexicalIndexError::Engine {
        operation,
        message: error.to_string(),
    }
}
