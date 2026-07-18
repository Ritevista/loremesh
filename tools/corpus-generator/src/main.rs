#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use loremesh_core::corpus::{
    CorpusManifest, ManifestArtifact, ManifestCodeReference, ManifestRelationship, ManifestSource,
};

const GENERATOR_VERSION: &str = "1";
const LARGE_THRESHOLD: u64 = 100 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "loremesh-corpus-gen")]
struct Cli {
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 100)]
    documents: u64,
    #[arg(long, default_value_t = 50)]
    issues: u64,
    #[arg(long, default_value_t = 500)]
    relationships: u64,
    #[arg(long, default_value = "10MB")]
    target_size: String,
    #[arg(long)]
    output: PathBuf,
    /// Add duplicate identities, missing files, and broken relationship candidates.
    #[arg(long)]
    quality_problems: bool,
    /// Required for requested outputs of 100 MiB or larger.
    #[arg(long)]
    allow_large: bool,
}

fn main() {
    if let Err(error) = run(&Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: &Cli) -> Result<()> {
    let target = parse_size(&cli.target_size)?;
    if target >= LARGE_THRESHOLD && !cli.allow_large {
        bail!("requested size requires --allow-large after reviewing disk capacity");
    }
    if cli.output.exists() {
        bail!(
            "refusing to replace existing output {}",
            cli.output.display()
        );
    }
    println!(
        "Generating deterministic corpus at {} (requested approximately {} bytes, seed {})",
        cli.output.display(),
        target,
        cli.seed
    );
    fs::create_dir_all(cli.output.join("corpus/documents")).context("creating document output")?;
    fs::create_dir_all(cli.output.join("issues")).context("creating issue output")?;
    let mut generator = Generator::new(cli.seed);
    let mut artifacts = Vec::new();
    let mut bytes_written = 0_u64;
    let required_documents = cli.documents.max(1);
    for index in 0..required_documents {
        bytes_written += write_document(&cli.output, index, &mut generator, &mut artifacts)?;
    }
    for index in 0..cli.issues {
        bytes_written += write_issue(&cli.output, index, &mut generator, &mut artifacts)?;
    }
    let mut padding_index = required_documents;
    while bytes_written < target {
        bytes_written +=
            write_document(&cli.output, padding_index, &mut generator, &mut artifacts)?;
        padding_index += 1;
    }
    let code_references = code_references(cli.seed);
    let mut relationships = relationships(cli.relationships, &artifacts, &code_references);
    if cli.quality_problems {
        inject_quality_problems(&mut artifacts, &mut relationships);
    }
    let manifest = CorpusManifest {
        schema_version: 1,
        name: format!("synthetic-scale-seed-{}", cli.seed),
        version: GENERATOR_VERSION.into(),
        sources: vec![ManifestSource {
            id: "synthetic".into(),
            kind: "generated".into(),
            origin: format!("loremesh-corpus-gen:{GENERATOR_VERSION}:seed:{}", cli.seed),
            revision: Some(GENERATOR_VERSION.into()),
        }],
        artifacts,
        code_references,
        expected_relationships: Vec::new(),
        relationships,
        external_analyses: Vec::new(),
    };
    fs::write(
        cli.output.join("corpus.json"),
        serde_json::to_vec_pretty(&manifest).context("serializing generated manifest")?,
    )
    .context("writing generated manifest")?;
    println!(
        "Generated {} artifacts and {} relationships (content bytes: {})",
        manifest.artifacts.len(),
        manifest.relationships.len(),
        bytes_written
    );
    Ok(())
}

fn write_document(
    root: &Path,
    index: u64,
    generator: &mut Generator,
    artifacts: &mut Vec<ManifestArtifact>,
) -> Result<u64> {
    const KINDS: [&str; 10] = [
        "feature_study",
        "interface_specification",
        "architecture_decision",
        "deployment_guide",
        "troubleshooting_guide",
        "security_analysis",
        "performance_study",
        "release_note",
        "operational_procedure",
        "code_specification",
    ];
    let kind_index = usize::try_from(index % KINDS.len() as u64)
        .context("document kind index does not fit this platform")?;
    let kind = KINDS[kind_index];
    let id = format!("document-{index:08}");
    let component = generator.pick(97);
    let version = generator.pick(23);
    let issue = generator.pick(50_000);
    let mut detail = String::new();
    for section in 0..256 {
        writeln!(
            detail,
            "Validation step {section} for component-{component:03} preserves source snapshot v{version}, checks dependency component-{:03}, configuration service.component_{component}.limit, test-{:06}, and build-{:06}.",
            (component + section) % 97,
            (issue + section) % 10_000,
            (generator.pick(10_000) + section) % 10_000,
        )
        .context("formatting generated document detail")?;
    }
    let content = format!(
        "# Synthetic {kind} {index}\n\nComponent: component-{component:03}\nVersion: v{version}\nOwner: team-{:02}\n\nThe component uses configuration key `service.component_{component}.limit` and depends on component-{:03}. This deterministic engineering record references issue-{issue:08}, test-{:06}, build-{:06}, and repository revision {:040x}.\n\n## Procedure\n\nValidate inputs, preserve immutable evidence, rebuild disposable indexes, and report stale or broken references without executing imported content.\n\n## Deterministic detail\n\n{detail}",
        generator.pick(31), generator.pick(97), generator.pick(10_000), generator.pick(10_000), generator.next()
    );
    let relative = format!("corpus/documents/{id}.md");
    fs::write(root.join(&relative), &content).context("writing generated document")?;
    artifacts.push(ManifestArtifact {
        id,
        source: "synthetic".into(),
        path: relative,
        title: format!("Synthetic {kind} {index}"),
        document_type: kind.into(),
        media_type: "text/markdown".into(),
        tags: vec!["synthetic".into(), format!("component-{component:03}")],
    });
    Ok(content.len() as u64)
}

fn write_issue(
    root: &Path,
    index: u64,
    generator: &mut Generator,
    artifacts: &mut Vec<ManifestArtifact>,
) -> Result<u64> {
    let id = format!("issue-{index:08}");
    let content = format!(
        "# Synthetic issue {index}\n\nStatus: {}\nTeam: team-{:02}\n\nThis record tracks a deterministic configuration and build investigation.\n",
        if generator.pick(7) == 0 { "stale" } else { "open" },
        generator.pick(31)
    );
    let relative = format!("issues/{id}.md");
    fs::write(root.join(&relative), &content).context("writing generated issue")?;
    artifacts.push(ManifestArtifact {
        id,
        source: "synthetic".into(),
        path: relative,
        title: format!("Synthetic issue {index}"),
        document_type: "issue".into(),
        media_type: "text/markdown".into(),
        tags: vec!["synthetic".into(), "issue".into()],
    });
    Ok(content.len() as u64)
}

fn code_references(seed: u64) -> Vec<ManifestCodeReference> {
    (0..16)
        .map(|index| ManifestCodeReference {
            id: format!("code-{index:04}"),
            repository: "synthetic/component-service".into(),
            revision: format!("{seed:040x}"),
            path: format!("src/component_{index:04}.rs"),
            symbol: Some(format!("process_component_{index}")),
            line_start: Some(1),
            line_end: Some(80),
        })
        .collect()
}

fn relationships(
    count: u64,
    artifacts: &[ManifestArtifact],
    code: &[ManifestCodeReference],
) -> Vec<ManifestRelationship> {
    if artifacts.is_empty() || code.is_empty() {
        return Vec::new();
    }
    let mut generated = Vec::new();
    let mut artifact_index = 0_usize;
    let mut code_index = 0_usize;
    for index in 0..count {
        let source = &artifacts[artifact_index].id;
        let target = &code[code_index].id;
        generated.push(ManifestRelationship {
            source: format!("artifact:{source}"),
            relation: if index % 17 == 0 {
                "candidate_for"
            } else {
                "implemented_by"
            }
            .into(),
            target: format!("code:{target}"),
            origin: if index % 17 == 0 {
                "inferred"
            } else {
                "deterministic"
            }
            .into(),
            verification: if index % 17 == 0 {
                "unreviewed"
            } else {
                "verified"
            }
            .into(),
            external_analysis: None,
            external_id: None,
        });
        artifact_index = (artifact_index + 1) % artifacts.len();
        code_index = (code_index + 1) % code.len();
    }
    generated
}

fn inject_quality_problems(
    artifacts: &mut Vec<ManifestArtifact>,
    relationships: &mut Vec<ManifestRelationship>,
) {
    if let Some(first) = artifacts.first().cloned() {
        artifacts.push(first.clone());
        artifacts.push(first);
    }
    artifacts.push(ManifestArtifact {
        id: "deliberately-missing-document".into(),
        source: "synthetic".into(),
        path: "corpus/documents/deliberately-missing.md".into(),
        title: "Deliberately missing synthetic document".into(),
        document_type: "troubleshooting_guide".into(),
        media_type: "text/markdown".into(),
        tags: vec!["synthetic".into(), "expected-missing".into()],
    });
    relationships.push(ManifestRelationship {
        source: "artifact:deliberately-missing-source".into(),
        relation: "incorrect_candidate".into(),
        target: "artifact:deliberately-missing-target".into(),
        origin: "inferred".into(),
        verification: "unreviewed".into(),
        external_analysis: None,
        external_id: None,
    });
}

fn parse_size(value: &str) -> Result<u64> {
    let normalized = value.trim().to_ascii_uppercase();
    for (suffix, multiplier) in [
        ("GB", 1024_u64.pow(3)),
        ("MB", 1024_u64.pow(2)),
        ("KB", 1024_u64),
        ("B", 1_u64),
    ] {
        if let Some(number) = normalized.strip_suffix(suffix) {
            let amount = number
                .trim()
                .parse::<u64>()
                .context("invalid target-size number")?;
            return amount
                .checked_mul(multiplier)
                .context("target size overflow");
        }
    }
    bail!("target size must use B, KB, MB, or GB, for example 500MB")
}

struct Generator {
    state: u64,
}

impl Generator {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.state
    }

    fn pick(&mut self, upper: u64) -> u64 {
        self.next() % upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_parser_is_binary_and_bounded() {
        assert_eq!(parse_size("100MB").expect("size"), 100 * 1024 * 1024);
        assert!(parse_size("large").is_err());
    }

    #[test]
    fn same_seed_and_arguments_are_logically_equivalent() {
        let first = tempfile::tempdir().expect("first parent");
        let second = tempfile::tempdir().expect("second parent");
        let arguments = |output| Cli {
            seed: 42,
            documents: 4,
            issues: 2,
            relationships: 8,
            target_size: "8KB".into(),
            output,
            allow_large: false,
            quality_problems: true,
        };
        let first_output = first.path().join("corpus");
        let second_output = second.path().join("corpus");
        run(&arguments(first_output.clone())).expect("first generation");
        run(&arguments(second_output.clone())).expect("second generation");
        assert_eq!(
            fs::read(first_output.join("corpus.json")).expect("first manifest"),
            fs::read(second_output.join("corpus.json")).expect("second manifest")
        );
    }
}
