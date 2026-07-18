use std::fs;
use std::path::Path;

use loremesh_core::index::{LexicalIndex, SearchQuery};
use loremesh_core::{
    ArtifactReference, Claim, ClaimId, EvidenceReference, Feedback, FeedbackId, FeedbackTarget,
    Finding, FindingId, KnowledgeScope, Trace, TraceId, VerificationStatus,
};
use loremesh_storage::{CorpusImportLimits, LocalRepository, TantivyIndex};

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/knowledge-base")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture directory");
    for entry in fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("fixture type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy fixture file");
        }
    }
}

#[test]
fn fixture_import_reports_health_and_is_idempotent() {
    let workspace = tempfile::tempdir().expect("workspace");
    LocalRepository::initialize(workspace.path()).expect("initialize");
    let mut repository = LocalRepository::open(workspace.path()).expect("open");
    let manifest = fixture().join("corpus.json");

    let first = repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::default())
        .expect("import fixture");
    assert_eq!(first.documents_discovered, 55);
    assert_eq!(first.documents_imported, 52);
    assert_eq!(first.snapshots_created, 52);
    assert_eq!(first.images, 2);
    assert_eq!(first.issues, 30);
    assert_eq!(first.code_references, 15);
    assert_eq!(first.relationships, 21);
    assert_eq!(first.external_relationships, 1);
    assert_eq!(
        first
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec!["duplicate_identity", "missing_file", "broken_relationship"]
    );

    let second = repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::default())
        .expect("repeat fixture import");
    assert_eq!(second.snapshots_created, 0);
    assert_eq!(second.unchanged_sources, 52);
    assert_eq!(repository.summary().expect("summary").artifacts, 52);
    assert_eq!(repository.relationships().expect("relationships").len(), 21);
}

#[test]
fn large_local_limits_explicitly_admit_scale_sized_manifests() {
    let corpus = tempfile::tempdir().expect("corpus copy");
    copy_tree(&fixture(), corpus.path());
    let manifest = corpus.path().join("corpus.json");
    let mut bytes = fs::read(&manifest).expect("read manifest");
    bytes.resize(2 * 1024 * 1024 + 1, b' ');
    fs::write(&manifest, bytes).expect("pad manifest");

    let default_workspace = tempfile::tempdir().expect("default workspace");
    LocalRepository::initialize(default_workspace.path()).expect("initialize default");
    let mut default_repository =
        LocalRepository::open(default_workspace.path()).expect("open default");
    assert!(default_repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::default())
        .expect_err("default limit must reject padded manifest")
        .to_string()
        .contains("use --allow-large"));

    let large_workspace = tempfile::tempdir().expect("large workspace");
    LocalRepository::initialize(large_workspace.path()).expect("initialize large");
    let mut large_repository = LocalRepository::open(large_workspace.path()).expect("open large");
    let imported = large_repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::large_local())
        .expect("large local import");
    assert_eq!(imported.documents_imported, 52);
}

#[test]
fn changed_source_preserves_old_snapshot_and_marks_old_artifact_stale() {
    let corpus = tempfile::tempdir().expect("corpus copy");
    copy_tree(&fixture(), corpus.path());
    let workspace = tempfile::tempdir().expect("workspace");
    LocalRepository::initialize(workspace.path()).expect("initialize");
    let mut repository = LocalRepository::open(workspace.path()).expect("open");
    let manifest = corpus.path().join("corpus.json");
    repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::default())
        .expect("first import");
    let old = repository
        .artifacts()
        .expect("artifacts")
        .into_iter()
        .find(|artifact| artifact.name == "Adaptive Retry Study")
        .expect("old artifact");
    let old_content = repository.artifact_content(&old).expect("old content");
    let evidence_start = old_content.find("relay adjusts").expect("evidence text") as u64;
    let evidence = EvidenceReference::new(
        ArtifactReference {
            artifact_id: old.id.clone(),
        },
        evidence_start,
        evidence_start + "relay adjusts".len() as u64,
        "original retry behavior",
        &old_content,
    )
    .expect("evidence");
    let finding = Finding::new(
        FindingId::deterministic("old-snapshot-finding"),
        "Original retry behavior",
        KnowledgeScope::SourceDerived,
        VerificationStatus::Verified,
        vec![Claim::new(
            ClaimId::deterministic("old-snapshot-claim"),
            "The original study describes relay adjustment.",
            vec![evidence],
        )
        .expect("claim")],
    )
    .expect("finding");
    repository
        .save_investigation(
            &finding,
            &Trace::new(TraceId::deterministic("old-snapshot-trace")),
        )
        .expect("persist old finding");

    fs::write(
        corpus
            .path()
            .join("corpus/feature-studies/adaptive-retry.md"),
        "# Adaptive Retry Study\n\nThe revised policy uses bounded jitter.\n",
    )
    .expect("change source");
    let changed = repository
        .import_corpus_manifest(&manifest, CorpusImportLimits::default())
        .expect("changed import");
    assert_eq!(changed.snapshots_created, 1);
    assert_eq!(repository.summary().expect("changed summary").snapshots, 53);
    assert!(repository
        .artifact_references_stale_snapshot(&old)
        .expect("stale check"));
    assert!(repository
        .artifact_content(&old)
        .expect("old content")
        .contains("retry windows"));
    let persisted = repository.findings().expect("findings");
    assert_eq!(
        persisted[0].claims[0].evidence[0].artifact.artifact_id,
        old.id
    );
}

#[test]
fn lexical_index_is_disposable_and_returns_canonical_ids() {
    let workspace = tempfile::tempdir().expect("workspace");
    LocalRepository::initialize(workspace.path()).expect("initialize");
    let mut repository = LocalRepository::open(workspace.path()).expect("open");
    repository
        .import_corpus_manifest(
            &fixture().join("corpus.json"),
            CorpusImportLimits::default(),
        )
        .expect("import fixture");
    let canonical_before = repository.summary().expect("summary");
    let documents = repository.index_documents().expect("index documents");
    assert_eq!(documents.len(), 50);
    let mut index = TantivyIndex::knowledge_for_workspace(workspace.path());
    assert_eq!(index.rebuild(documents.clone()).expect("build").indexed, 50);
    let hits = index
        .search(&SearchQuery::new("bounded retry", 10).expect("query"))
        .expect("search");
    assert!(!hits.is_empty());
    assert!(repository
        .artifacts()
        .expect("artifacts")
        .iter()
        .any(|artifact| artifact.id == hits[0].artifact_id));

    index.drop_index().expect("drop index");
    assert_eq!(
        repository.summary().expect("summary after drop"),
        canonical_before
    );
    assert_eq!(index.rebuild(documents).expect("rebuild").indexed, 50);
}

#[test]
fn relationship_feedback_survives_provider_indirection() {
    let workspace = tempfile::tempdir().expect("workspace");
    LocalRepository::initialize(workspace.path()).expect("initialize");
    let mut repository = LocalRepository::open(workspace.path()).expect("open");
    repository
        .import_corpus_manifest(
            &fixture().join("corpus.json"),
            CorpusImportLimits::default(),
        )
        .expect("import fixture");
    let relationship = repository
        .relationships()
        .expect("relationships")
        .into_iter()
        .find(|relationship| relationship.external_provenance.is_some())
        .expect("external relationship");
    let feedback = Feedback::new(
        FeedbackId::deterministic("fixture-review"),
        FeedbackTarget::Relationship(relationship.id),
        KnowledgeScope::Personal,
        "The external candidate needs human review.",
        VerificationStatus::Disputed,
    )
    .expect("feedback");
    repository.save_feedback(&feedback).expect("save feedback");
    let organization_feedback = Feedback::new(
        FeedbackId::deterministic("fixture-organization-review"),
        feedback.target,
        KnowledgeScope::Organization,
        "Reviewed proposal: reject this relationship.",
        VerificationStatus::Rejected,
    )
    .expect("organization feedback");
    repository
        .save_feedback(&organization_feedback)
        .expect("save organization feedback");
}

#[test]
fn traversal_and_malformed_utf8_fail_without_escape() {
    let corpus = tempfile::tempdir().expect("corpus copy");
    copy_tree(&fixture(), corpus.path());
    let manifest_path = corpus.path().join("corpus.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    manifest["artifacts"][0]["path"] = "../outside.md".into();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write traversal manifest");
    let workspace = tempfile::tempdir().expect("workspace");
    LocalRepository::initialize(workspace.path()).expect("initialize");
    let mut repository = LocalRepository::open(workspace.path()).expect("open");
    assert!(repository
        .import_corpus_manifest(&manifest_path, CorpusImportLimits::default())
        .expect_err("reject traversal")
        .to_string()
        .contains("unsafe corpus-relative path"));

    copy_tree(&fixture(), corpus.path());
    fs::write(
        corpus
            .path()
            .join("corpus/feature-studies/adaptive-retry.md"),
        [0xff, 0xfe],
    )
    .expect("invalid UTF-8");
    assert!(repository
        .import_corpus_manifest(&manifest_path, CorpusImportLimits::default())
        .is_err());
}
