#![forbid(unsafe_code)]

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use loremesh_core::{
    ArtifactReference, Claim, ClaimId, EdgeOrigin, EvidenceReference, Finding, FindingId,
    KnowledgeScope, ReportId, Trace, TraceEdge, TraceEdgeId, TraceId, TraceNode, TraceNodeId,
    TraceNodeKind, VerificationStatus,
};
use loremesh_report::{Metric, Report, ReportBlock, ReportSection, TableModel};
use loremesh_storage::LocalRepository;
use tracing::info;

const DEMO_MARKDOWN: &str = "# Build investigation\n\nThe retry policy uses three attempts before reporting failure.\n\nThis fixture is generic and deterministic.\n";
const EVIDENCE_TEXT: &str = "three attempts";

#[derive(Debug, Parser)]
#[command(
    name = "loremesh",
    version,
    about = "Local-first evidence-backed engineering investigations"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommand,
    },
    Demo {
        #[command(subcommand)]
        command: DemoCommand,
    },
    /// Open the offline terminal dashboard.
    Tui,
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Verify schema and immutable object integrity.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    /// Create or validate a workspace.
    Init { path: PathBuf },
    /// Display content-safe entity counts.
    Status,
}

#[derive(Debug, Subcommand)]
enum ArtifactCommand {
    /// Import one UTF-8 Markdown file.
    Import { file: PathBuf },
}

#[derive(Debug, Subcommand)]
enum DemoCommand {
    /// Create deterministic sample content and an evidence trace.
    Seed,
}

#[derive(Debug, Subcommand)]
enum ReportCommand {
    /// Export the current workspace report.
    Export {
        #[arg(long, value_enum)]
        format: ExportFormat,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormat {
    Json,
    Csv,
    Markdown,
    Html,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Markdown => "md",
            Self::Html => "html",
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .without_time()
        .try_init()
        .ok();
    match Cli::parse().command {
        Command::Workspace {
            command: WorkspaceCommand::Init { path },
        } => {
            let workspace = LocalRepository::initialize(&path)
                .with_context(|| format!("could not initialize workspace at {}", path.display()))?;
            println!(
                "Initialized workspace {} ({})",
                workspace.root.display(),
                workspace.id
            );
        }
        Command::Workspace {
            command: WorkspaceCommand::Status,
        } => print_status(&open_current()?)?,
        Command::Artifact {
            command: ArtifactCommand::Import { file },
        } => {
            let mut repository = open_current()?;
            let result = repository
                .import_markdown(&file)
                .with_context(|| format!("could not import {}", file.display()))?;
            println!(
                "Artifact: {}\nSnapshot: {}\nStatus: {}",
                result.artifact.id,
                result.snapshot.id,
                if result.inserted {
                    "imported"
                } else {
                    "already present"
                }
            );
        }
        Command::Demo {
            command: DemoCommand::Seed,
        } => seed_demo(&current_root()?)?,
        Command::Tui => {
            let repository = open_current()?;
            let workspace = repository.workspace()?;
            let artifacts = repository.artifacts()?;
            let findings = repository.findings()?;
            let trace = findings
                .first()
                .map(|finding| repository.trace_for(finding))
                .transpose()?;
            let view = loremesh_tui::DashboardView::from_domain(
                &workspace.name,
                &artifacts,
                &findings,
                trace.as_ref(),
            );
            loremesh_tui::run(&view).context("terminal dashboard failed")?;
        }
        Command::Report {
            command: ReportCommand::Export { format, output },
        } => export_report(format, output.as_deref())?,
        Command::Doctor => {
            let repository = open_current()?;
            repository.doctor()?;
            println!("Workspace is healthy");
        }
    }
    Ok(())
}

fn current_root() -> Result<PathBuf> {
    std::env::current_dir().context("could not determine current directory")
}
fn open_current() -> Result<LocalRepository> {
    let root = current_root()?;
    LocalRepository::open(&root)
        .with_context(|| format!("could not open workspace at {}", root.display()))
}

fn print_status(repository: &LocalRepository) -> Result<()> {
    let workspace = repository.workspace()?;
    let summary = repository.summary()?;
    println!(
        "Workspace: {}\nID: {}\nSources: {}\nSnapshots: {}\nArtifacts: {}\nFindings: {}",
        workspace.name,
        workspace.id,
        summary.sources,
        summary.snapshots,
        summary.artifacts,
        summary.findings
    );
    Ok(())
}

fn seed_demo(root: &Path) -> Result<()> {
    let sample = root.join("sample.md");
    if sample.exists() {
        let existing =
            fs::read_to_string(&sample).context("could not inspect existing sample.md")?;
        if existing != DEMO_MARKDOWN {
            bail!("refusing to overwrite existing sample.md with different content");
        }
    } else {
        fs::write(&sample, DEMO_MARKDOWN).context("could not write deterministic demo fixture")?;
    }
    let mut repository = LocalRepository::open(root)?;
    let imported = repository.import_markdown(&sample)?;
    let start = DEMO_MARKDOWN
        .find(EVIDENCE_TEXT)
        .context("demo evidence text is missing")? as u64;
    let evidence = EvidenceReference::new(
        ArtifactReference {
            artifact_id: imported.artifact.id.clone(),
        },
        start,
        start + EVIDENCE_TEXT.len() as u64,
        "retry count statement",
        DEMO_MARKDOWN,
    )?;
    let claim = Claim::new(
        ClaimId::deterministic("demo-retry-claim"),
        "The retry policy permits three attempts.",
        vec![evidence.clone()],
    )?;
    let finding = Finding::new(
        FindingId::deterministic("demo-retry-finding"),
        "Retry behaviour is explicitly bounded",
        KnowledgeScope::SourceDerived,
        VerificationStatus::Verified,
        vec![claim],
    )?;
    let finding_node = TraceNodeId::deterministic("demo-finding-node");
    let evidence_node = TraceNodeId::deterministic("demo-evidence-node");
    let snapshot_node = TraceNodeId::deterministic("demo-snapshot-node");
    let mut trace = Trace::new(TraceId::deterministic("demo-trace"));
    trace.add_node(TraceNode::new(
        finding_node.clone(),
        "Finding",
        TraceNodeKind::Finding(finding.id.clone()),
    )?)?;
    trace.add_node(TraceNode::new(
        evidence_node.clone(),
        "Evidence: retry count statement",
        TraceNodeKind::Evidence(format!(
            "{}:{}-{}",
            evidence.artifact.artifact_id, evidence.start, evidence.end
        )),
    )?)?;
    trace.add_node(TraceNode::new(
        snapshot_node.clone(),
        "Immutable source snapshot",
        TraceNodeKind::Snapshot(imported.snapshot.id.clone()),
    )?)?;
    trace.add_edge(TraceEdge {
        id: TraceEdgeId::deterministic("demo-finding-evidence"),
        from: finding_node,
        to: evidence_node.clone(),
        origin: EdgeOrigin::Manual,
        status: VerificationStatus::Verified,
    })?;
    trace.add_edge(TraceEdge {
        id: TraceEdgeId::deterministic("demo-evidence-snapshot"),
        from: evidence_node,
        to: snapshot_node,
        origin: EdgeOrigin::Deterministic,
        status: VerificationStatus::Verified,
    })?;
    repository.save_investigation(&finding, &trace)?;
    println!(
        "Seeded deterministic demo\nArtifact: {}\nFinding: {}\nTrace: {}",
        imported.artifact.id, finding.id, trace.id
    );
    Ok(())
}

fn workspace_report(repository: &LocalRepository) -> Result<Report> {
    let workspace = repository.workspace()?;
    let summary = repository.summary()?;
    let artifacts = repository.artifacts()?;
    let findings = repository.findings()?;
    let artifact_table = TableModel::new(
        "Artifacts",
        vec!["Name".into(), "Kind".into()],
        artifacts
            .iter()
            .map(|artifact| vec![artifact.name.clone(), "Markdown".into()])
            .collect(),
    )?;
    let finding_table = TableModel::new(
        "Findings",
        vec!["Title".into(), "Status".into(), "Scope".into()],
        findings
            .iter()
            .map(|finding| {
                vec![
                    finding.title.clone(),
                    format!("{:?}", finding.status),
                    format!("{:?}", finding.scope),
                ]
            })
            .collect(),
    )?;
    let sections = vec![
        ReportSection::new(
            "Workspace summary",
            vec![
                ReportBlock::Metric(Metric::new(
                    "Artifacts",
                    summary.artifacts.to_string(),
                    None,
                )?),
                ReportBlock::Metric(Metric::new("Findings", summary.findings.to_string(), None)?),
            ],
        )?,
        ReportSection::new("Artifacts", vec![ReportBlock::Table(artifact_table)])?,
        ReportSection::new("Findings", vec![ReportBlock::Table(finding_table)])?,
    ];
    Report::new(
        ReportId::deterministic(workspace.id.as_str()),
        format!("LoreMesh workspace: {}", workspace.name),
        sections,
    )
    .map_err(Into::into)
}

fn export_report(format: ExportFormat, requested: Option<&Path>) -> Result<()> {
    let root = current_root()?;
    let repository = LocalRepository::open(&root)?;
    let report = workspace_report(&repository)?;
    let relative = requested.map_or_else(
        || PathBuf::from(format!("report.{}", format.extension())),
        Path::to_path_buf,
    );
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || relative.starts_with(".loremesh")
    {
        bail!("export output must be a safe workspace-relative path outside .loremesh");
    }
    let output = root.join(&relative);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("could not create export directory")?;
    }
    let rendered = match format {
        ExportFormat::Json => loremesh_report::render_json(&report)?,
        ExportFormat::Csv => loremesh_report::render_csv(&report)?,
        ExportFormat::Markdown => loremesh_report::render_markdown(&report),
        ExportFormat::Html => loremesh_report::render_html(&report),
    };
    let temporary = output.with_extension(format!("{}.tmp", format.extension()));
    fs::write(&temporary, rendered.as_bytes())
        .with_context(|| format!("could not write temporary export {}", temporary.display()))?;
    fs::rename(&temporary, &output)
        .with_context(|| format!("could not finalize export {}", output.display()))?;
    info!(format = ?format, bytes = rendered.len(), "report exported");
    println!("Exported {}", relative.display());
    Ok(())
}
