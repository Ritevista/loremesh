use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn complete_offline_workflow_exports_all_formats() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let workspace = temporary.path().join("workspace");
    Command::cargo_bin("loremesh")
        .expect("binary")
        .args(["workspace", "init"])
        .arg(&workspace)
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized workspace"));
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["demo", "seed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Seeded deterministic demo"));
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["workspace", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Artifacts: 1").and(predicate::str::contains("Findings: 1")),
        );
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("healthy"));
    for format in ["json", "csv", "markdown", "html"] {
        Command::cargo_bin("loremesh")
            .expect("binary")
            .current_dir(&workspace)
            .args([
                "report",
                "export",
                "--format",
                format,
                "--output",
                &format!("out/report.{format}"),
            ])
            .assert()
            .success();
    }
    let json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace.join("out/report.json")).expect("JSON export"),
    )
    .expect("valid JSON");
    assert_eq!(json["title"], "LoreMesh workspace: workspace");
    assert!(fs::read_to_string(workspace.join("out/report.html"))
        .expect("HTML export")
        .contains("<!doctype html>"));
}

#[test]
fn commands_fail_usefully_outside_workspace() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(temporary.path())
        .args(["workspace", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "run `loremesh workspace init <path>`",
        ));
}

#[test]
fn export_rejects_traversal() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    Command::cargo_bin("loremesh")
        .expect("binary")
        .args(["workspace", "init"])
        .arg(temporary.path())
        .assert()
        .success();
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(temporary.path())
        .args([
            "report",
            "export",
            "--format",
            "json",
            "--output",
            "../leak.json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("safe workspace-relative path"));
    assert!(!temporary
        .path()
        .parent()
        .expect("parent")
        .join("leak.json")
        .exists());
}

#[test]
fn corpus_import_index_search_drop_and_rebuild_are_offline() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let workspace = temporary.path().join("workspace");
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/knowledge-base/corpus.json");
    Command::cargo_bin("loremesh")
        .expect("binary")
        .args(["workspace", "init"])
        .arg(&workspace)
        .assert()
        .success();
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["corpus", "import"])
        .arg(&manifest)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Documents imported:")
                .and(predicate::str::contains("52"))
                .and(predicate::str::contains("broken_relationship")),
        );
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["index", "build", "knowledge"])
        .assert()
        .success()
        .stdout(predicate::str::contains("50 document(s)"));
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["index", "search", "bounded retry"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Adaptive Retry Study"));
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["index", "drop", "knowledge"])
        .assert()
        .success()
        .stdout(predicate::str::contains("canonical knowledge is unchanged"));
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["workspace", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Artifacts: 52"));
}

#[test]
fn corpus_open_discovers_directory_imports_and_builds_index() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let workspace = temporary.path().join("workspace");
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/knowledge-base");
    Command::cargo_bin("loremesh")
        .expect("binary")
        .args(["workspace", "init"])
        .arg(&workspace)
        .assert()
        .success();
    Command::cargo_bin("loremesh")
        .expect("binary")
        .current_dir(&workspace)
        .args(["corpus", "open"])
        .arg(&corpus)
        .arg("--no-tui")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Documents imported:").and(predicate::str::contains(
                "Knowledge index ready: 50 document(s)",
            )),
        );
    assert!(workspace.join(".loremesh/indexes/knowledge").is_dir());
}
