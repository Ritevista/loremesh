#![forbid(unsafe_code)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use loremesh_core::corpus::{
    CorpusManifest, ManifestArtifact, ManifestCodeReference, ManifestRelationship, ManifestSource,
};
use serde::{Deserialize, Serialize};

const PROFILE_JSON: &str = include_str!("../profiles/kubernetes-feature-sample.json");

#[derive(Debug, Parser)]
#[command(name = "loremesh-public-corpus")]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Validate immutable revisions and curated paths without network access.
    VerifyProfile,
    /// Explicitly fetch pinned public sources and build generic corpus inputs.
    Build {
        #[arg(long, default_value = "target/test-corpora/kubernetes-feature-sample")]
        output: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
struct Profile {
    name: String,
    version: String,
    licenses: Vec<LicenseEntry>,
    repositories: Repositories,
    features: Vec<Feature>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LicenseEntry {
    project: String,
    #[serde(rename = "license")]
    spdx: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Repositories {
    enhancements: Repository,
    kubernetes: Repository,
}

#[derive(Debug, Deserialize)]
struct Repository {
    url: String,
    revision: String,
}

#[derive(Debug, Deserialize)]
struct Feature {
    id: String,
    title: String,
    kep_path: String,
    tracking_issue: u64,
    code_path: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let profile: Profile =
        serde_json::from_str(PROFILE_JSON).context("invalid embedded profile")?;
    validate_profile(&profile)?;
    match Cli::parse().command {
        Action::VerifyProfile => println!(
            "Profile {} version {} is valid: {} pinned features, {} license records",
            profile.name,
            profile.version,
            profile.features.len(),
            profile.licenses.len()
        ),
        Action::Build { output } => build(&profile, &output)?,
    }
    Ok(())
}

fn validate_profile(profile: &Profile) -> Result<()> {
    if !(3..=5).contains(&profile.features.len()) {
        bail!("public profile must curate between 3 and 5 features");
    }
    for repository in [
        &profile.repositories.enhancements,
        &profile.repositories.kubernetes,
    ] {
        if repository.revision.len() != 40
            || !repository
                .revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !repository.url.starts_with("https://github.com/")
        {
            bail!("repository must use a GitHub URL and immutable 40-character revision");
        }
    }
    for feature in &profile.features {
        safe_relative(&feature.kep_path)?;
        safe_relative(&feature.code_path)?;
    }
    if profile.licenses.iter().any(|license| {
        license.project.trim().is_empty()
            || license.spdx != "Apache-2.0"
            || !license.url.starts_with("https://github.com/")
    }) {
        bail!("every public source requires an Apache-2.0 license record");
    }
    Ok(())
}

fn build(profile: &Profile, output: &Path) -> Result<()> {
    if output.exists() {
        bail!("refusing to replace existing output {}", output.display());
    }
    println!(
        "Building explicit public corpus {} at {} from pinned revisions",
        profile.name,
        output.display()
    );
    fs::create_dir_all(output).context("creating public corpus output")?;
    let checkout = tempfile::tempdir().context("creating temporary public source cache")?;
    let enhancements = checkout.path().join("enhancements");
    let kubernetes = checkout.path().join("kubernetes");
    fetch(&profile.repositories.enhancements, &enhancements)?;
    fetch(&profile.repositories.kubernetes, &kubernetes)?;
    let manifest = transform(profile, &enhancements, &kubernetes, output)?;
    fs::write(
        output.join("corpus.json"),
        serde_json::to_vec_pretty(&manifest).context("serializing public corpus manifest")?,
    )
    .context("writing public corpus manifest")?;
    fs::write(
        output.join("UPSTREAM-LICENSES.json"),
        serde_json::to_vec_pretty(&profile.licenses)?,
    )
    .context("writing upstream license records")?;
    println!(
        "Built {} curated features; no mutable branch was used",
        profile.features.len()
    );
    Ok(())
}

fn fetch(repository: &Repository, destination: &Path) -> Result<()> {
    run_git(
        destination.parent().context("checkout parent missing")?,
        [
            "init",
            destination.to_str().context("non-UTF-8 checkout path")?,
        ],
    )?;
    run_git(
        destination,
        ["remote", "add", "origin", repository.url.as_str()],
    )?;
    run_git(
        destination,
        ["fetch", "--depth=1", "origin", repository.revision.as_str()],
    )?;
    run_git(destination, ["checkout", "--detach", "FETCH_HEAD"])
}

fn run_git<const N: usize>(directory: &Path, arguments: [&str; N]) -> Result<()> {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .status()
        .context("could not execute git for explicit public corpus fetch")?;
    if !status.success() {
        bail!("git failed while fetching a pinned public source");
    }
    Ok(())
}

fn transform(
    profile: &Profile,
    enhancements: &Path,
    kubernetes: &Path,
    output: &Path,
) -> Result<CorpusManifest> {
    let mut artifacts = Vec::new();
    let mut code_references = Vec::new();
    let mut relationships = Vec::new();
    for feature in &profile.features {
        let feature_dir = output.join("corpus/features").join(&feature.id);
        fs::create_dir_all(&feature_dir).context("creating public feature directory")?;
        let proposal_source = enhancements.join(&feature.kep_path).join("README.md");
        let proposal_path = format!("corpus/features/{}/proposal.md", feature.id);
        fs::copy(&proposal_source, output.join(&proposal_path))
            .with_context(|| format!("copying pinned proposal for {}", feature.id))?;
        let issue_path = format!("issues/{}.md", feature.id);
        fs::create_dir_all(output.join("issues")).context("creating public issues directory")?;
        fs::write(
            output.join(&issue_path),
            format!(
                "# Tracking issue for {}\n\nUpstream: https://github.com/kubernetes/enhancements/issues/{}\n\nPinned corpus metadata; issue content is not mirrored.\n",
                feature.title, feature.tracking_issue
            ),
        )
        .context("writing public issue reference")?;
        let code_output = format!("code/kubernetes/{}", feature.code_path);
        if let Some(parent) = output.join(&code_output).parent() {
            fs::create_dir_all(parent).context("creating public code directory")?;
        }
        fs::copy(
            kubernetes.join(&feature.code_path),
            output.join(&code_output),
        )
        .with_context(|| format!("copying pinned code for {}", feature.id))?;
        let proposal_id = format!("{}-proposal", feature.id);
        let issue_id = format!("{}-issue", feature.id);
        let code_id = format!("{}-code", feature.id);
        artifacts.push(artifact(
            &proposal_id,
            "enhancements",
            &proposal_path,
            &feature.title,
            "feature_study",
        ));
        artifacts.push(artifact(
            &issue_id,
            "issues",
            &issue_path,
            &format!("{} tracking issue", feature.title),
            "issue",
        ));
        code_references.push(ManifestCodeReference {
            id: code_id.clone(),
            repository: "kubernetes/kubernetes".into(),
            revision: profile.repositories.kubernetes.revision.clone(),
            path: feature.code_path.clone(),
            symbol: None,
            line_start: None,
            line_end: None,
        });
        relationships.push(relation(
            &format!("artifact:{proposal_id}"),
            "tracked_by",
            &format!("artifact:{issue_id}"),
        ));
        relationships.push(relation(
            &format!("artifact:{issue_id}"),
            "implemented_by",
            &format!("code:{code_id}"),
        ));
    }
    Ok(CorpusManifest {
        schema_version: 1,
        name: profile.name.clone(),
        version: profile.version.clone(),
        sources: vec![
            source("enhancements", "git", &profile.repositories.enhancements),
            source(
                "issues",
                "public_reference",
                &profile.repositories.enhancements,
            ),
        ],
        artifacts,
        code_references,
        expected_relationships: relationships.clone(),
        relationships,
        external_analyses: Vec::new(),
    })
}

fn source(id: &str, kind: &str, repository: &Repository) -> ManifestSource {
    ManifestSource {
        id: id.into(),
        kind: kind.into(),
        origin: repository.url.trim_end_matches(".git").into(),
        revision: Some(repository.revision.clone()),
    }
}

fn artifact(id: &str, source: &str, path: &str, title: &str, kind: &str) -> ManifestArtifact {
    ManifestArtifact {
        id: id.into(),
        source: source.into(),
        path: path.into(),
        title: title.into(),
        document_type: kind.into(),
        media_type: "text/markdown".into(),
        tags: vec!["public".into(), "kubernetes".into()],
    }
}

fn relation(source: &str, relation: &str, target: &str) -> ManifestRelationship {
    ManifestRelationship {
        source: source.into(),
        relation: relation.into(),
        target: target.into(),
        origin: "imported".into(),
        verification: "verified".into(),
        external_analysis: None,
        external_id: None,
    }
}

fn safe_relative(value: &str) -> Result<()> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("profile path must be normalized and relative: {value}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_profile_is_complete_and_offline_verifiable() {
        let profile: Profile = serde_json::from_str(PROFILE_JSON).expect("embedded profile");
        validate_profile(&profile).expect("valid pinned public profile");
        assert_eq!(profile.features.len(), 4);
        assert_eq!(profile.licenses.len(), 2);
    }
}
