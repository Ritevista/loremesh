#![forbid(unsafe_code)]

mod workbench;

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use loremesh_core::index::{LexicalIndex, SearchQuery};
use loremesh_core::{
    ArtifactReference, Claim, ClaimId, EdgeOrigin, EvidenceReference, Finding, FindingId,
    KnowledgeScope, ReportId, Trace, TraceEdge, TraceEdgeId, TraceId, TraceNode, TraceNodeId,
    TraceNodeKind, VerificationStatus,
};
use loremesh_report::{Metric, Report, ReportBlock, ReportSection, TableModel};
use loremesh_storage::{CorpusImportLimits, CorpusImportResult, LocalRepository, TantivyIndex};
use loremesh_tui::grid::DataGrid;
use loremesh_tui::{
    CommandHandler, CommandResponse, InputMode, SaveFormat, SlashCommand, ViewContent,
};
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
    Corpus {
        #[command(subcommand)]
        command: CorpusCommand,
    },
    Index {
        #[command(subcommand)]
        command: IndexCommand,
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
enum CorpusCommand {
    /// Import a local schema-versioned corpus manifest without network access.
    Import {
        manifest: PathBuf,
        /// Opt in to bounded limits suitable for the provided local 100 MB–2 GB scale corpora.
        #[arg(long)]
        allow_large: bool,
    },
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Build or replace the disposable knowledge index from canonical artifacts.
    Build { kind: IndexKind },
    /// Display disposable index lifecycle state.
    Status,
    /// Search the local knowledge index.
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Delete only a disposable index.
    Drop { kind: IndexKind },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IndexKind {
    Knowledge,
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
        Command::Corpus { command } => run_corpus(command)?,
        Command::Index { command } => run_index(command)?,
        Command::Demo {
            command: DemoCommand::Seed,
        } => seed_demo(&current_root()?)?,
        Command::Tui => {
            let root = current_root()?;
            let repository = LocalRepository::open(&root)?;
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
            let mut handler = TuiCommandHandler::new(root);
            loremesh_tui::run(&view, &mut handler).context("terminal dashboard failed")?;
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

fn run_corpus(command: CorpusCommand) -> Result<()> {
    match command {
        CorpusCommand::Import {
            manifest,
            allow_large,
        } => {
            let mut repository = open_current()?;
            let limits = if allow_large {
                CorpusImportLimits::large_local()
            } else {
                CorpusImportLimits::default()
            };
            let imported = repository
                .import_corpus_manifest(&manifest, limits)
                .with_context(|| {
                    format!("could not import corpus manifest {}", manifest.display())
                })?;
            print_corpus_import(&imported)
        }
    }
}

fn run_index(command: IndexCommand) -> Result<()> {
    let root = current_root()?;
    let repository = LocalRepository::open(&root)?;
    let mut index = TantivyIndex::knowledge_for_workspace(&root);
    match command {
        IndexCommand::Build {
            kind: IndexKind::Knowledge,
        } => {
            let result = index.rebuild(repository.index_documents()?)?;
            println!("Knowledge index ready: {} document(s)", result.indexed);
        }
        IndexCommand::Status => {
            let canonical = repository.summary()?;
            let status = index.status()?;
            println!(
                "Knowledge index: {:?}\nSchema: {}\nIndexed documents: {}\nCanonical artifacts: {}",
                status.state, status.schema_version, status.documents, canonical.artifacts
            );
        }
        IndexCommand::Search { query, limit } => {
            for hit in index.search(&SearchQuery::new(query, limit)?)? {
                println!("{}\t{}\t{:.3}", hit.artifact_id, hit.title, hit.score);
            }
        }
        IndexCommand::Drop {
            kind: IndexKind::Knowledge,
        } => {
            index.drop_index()?;
            println!("Dropped disposable knowledge index; canonical knowledge is unchanged");
        }
    }
    Ok(())
}

fn print_corpus_import(imported: &CorpusImportResult) -> Result<()> {
    let diagnostics = TableModel::new(
        "Diagnostics",
        vec![
            "Severity".into(),
            "Code".into(),
            "Subject".into(),
            "Message".into(),
        ],
        imported
            .diagnostics
            .iter()
            .map(|diagnostic| {
                vec![
                    format!("{:?}", diagnostic.severity),
                    diagnostic.code.clone(),
                    diagnostic.subject.clone(),
                    diagnostic.message.clone(),
                ]
            })
            .collect(),
    )?;
    let report = Report::new(
        ReportId::deterministic(format!("corpus-import:{}", imported.corpus_name)),
        format!("Corpus import: {}", imported.corpus_name),
        vec![
            ReportSection::new(
                "Imported",
                vec![
                    ReportBlock::Metric(Metric::new(
                        "Documents discovered",
                        imported.documents_discovered.to_string(),
                        None,
                    )?),
                    ReportBlock::Metric(Metric::new(
                        "Documents imported",
                        imported.documents_imported.to_string(),
                        None,
                    )?),
                    ReportBlock::Metric(Metric::new(
                        "Snapshots created",
                        imported.snapshots_created.to_string(),
                        None,
                    )?),
                    ReportBlock::Metric(Metric::new(
                        "Unchanged sources",
                        imported.unchanged_sources.to_string(),
                        None,
                    )?),
                    ReportBlock::Metric(Metric::new("Images", imported.images.to_string(), None)?),
                    ReportBlock::Metric(Metric::new("Issues", imported.issues.to_string(), None)?),
                    ReportBlock::Metric(Metric::new(
                        "Code references",
                        imported.code_references.to_string(),
                        None,
                    )?),
                    ReportBlock::Metric(Metric::new(
                        "Relationships",
                        imported.relationships.to_string(),
                        None,
                    )?),
                ],
            )?,
            ReportSection::new("Problems", vec![ReportBlock::Table(diagnostics)])?,
        ],
    )?;
    print!("{}", loremesh_report::render_markdown(&report));
    Ok(())
}

struct TuiCommandHandler {
    root: PathBuf,
    grid: Option<DataGrid>,
    grid_source: Option<PathBuf>,
    shell_enabled: bool,
    code_document: Option<loremesh_tui::browser::CodeDocument>,
    shell_session: Option<workbench::PtySession>,
    pending_input_mode: Option<InputMode>,
}

impl TuiCommandHandler {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            grid: None,
            grid_source: None,
            shell_enabled: false,
            code_document: None,
            shell_session: None,
            pending_input_mode: None,
        }
    }
}

impl CommandHandler for TuiCommandHandler {
    fn execute(&mut self, command: &SlashCommand, active: &ViewContent) -> CommandResponse {
        let result: Result<(String, Option<ViewContent>)> = match command {
            SlashCommand::Services => Ok(message_result("Services", "storage: local SQLite\ngraph: not configured\nmodel: not configured\nnetwork: offline")),
            SlashCommand::Model => Ok(message_result("Model", "No model configured. LoreMesh remains fully usable offline.")),
            SlashCommand::Context => Ok(message_result("Context", &format!(
                "Local context preview: '{}' with {} paragraph(s) and {} table row(s). Nothing was transmitted.",
                active.title,
                active.paragraphs.len(),
                active.table.as_ref().map_or(0, |table| table.rows.len())
            ))),
            SlashCommand::Compact => Ok(message_result("Compact", "Compaction requires an optional configured model; no content was transmitted or changed.")),
            SlashCommand::Save { format, output } => save_active_view(&self.root, active, *format, output.as_deref()).map(|message| message_result("Save result", &message)),
            SlashCommand::Table(command) => self.table_command(command),
            SlashCommand::Chart { kind, label_column, value_column } => self.chart_command(*kind, label_column, value_column),
            SlashCommand::Shell(command) => self.shell_command(command),
            SlashCommand::Browser(command) => self.browser_command(command),
            SlashCommand::Demo(kind) => Ok(self.demo_command(*kind)),
            SlashCommand::Help | SlashCommand::View(_) | SlashCommand::Clear | SlashCommand::Quit => Ok(message_result("Workbench", "Command handled by the workbench shell.")),
        };
        match result {
            Ok((message, content)) => CommandResponse {
                message,
                content,
                input_mode: self.pending_input_mode.take(),
            },
            Err(error) => CommandResponse {
                message: format!("Command failed: {error:#}"),
                content: Some(message_view("Command failed", &format!("{error:#}"))),
                input_mode: self.pending_input_mode.take(),
            },
        }
    }

    fn poll(&mut self) -> Option<CommandResponse> {
        self.poll_shell()
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.resize_shell(rows, cols);
    }
}

fn message_result(title: &str, message: &str) -> (String, Option<ViewContent>) {
    (message.into(), Some(message_view(title, message)))
}

fn message_view(title: &str, message: &str) -> ViewContent {
    ViewContent {
        title: title.into(),
        paragraphs: vec![message.into()],
        table: None,
        chart: None,
        mermaid: None,
        d2: None,
    }
}

fn report_from_view(active: &ViewContent) -> Result<Report> {
    let mut blocks = active
        .paragraphs
        .iter()
        .cloned()
        .map(ReportBlock::Paragraph)
        .collect::<Vec<_>>();
    if let Some(chart) = &active.chart {
        blocks.push(ReportBlock::Paragraph(chart.render_text(80)));
    }
    if let Some(table) = &active.table {
        blocks.push(ReportBlock::Table(TableModel::new(
            active.title.clone(),
            table.columns.clone(),
            table.rows.clone(),
        )?));
    }
    Report::new(
        ReportId::deterministic(active.title.as_bytes()),
        active.title.clone(),
        vec![ReportSection::new("Current view", blocks)?],
    )
    .map_err(Into::into)
}

fn slug(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let compact = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if compact.is_empty() {
        "view".into()
    } else {
        compact
    }
}

fn safe_workspace_output(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || relative.starts_with(".loremesh")
    {
        bail!("output must be a safe workspace-relative path outside .loremesh");
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if let Component::Normal(part) = component {
            current.push(part);
            if let Ok(metadata) = fs::symlink_metadata(&current) {
                if metadata.file_type().is_symlink() {
                    bail!("output path must not traverse symbolic links");
                }
            }
        }
    }
    Ok(current)
}

fn save_active_view(
    root: &Path,
    active: &ViewContent,
    format: SaveFormat,
    requested: Option<&str>,
) -> Result<String> {
    let extension = match format {
        SaveFormat::Markdown | SaveFormat::MarkdownMermaid | SaveFormat::MarkdownD2 => "md",
        SaveFormat::Csv => "csv",
        SaveFormat::Html => "html",
        SaveFormat::Png => {
            bail!(
                "PNG export requires a configured local renderer; use md, markdown-mermaid, markdown-d2, csv, or html"
            )
        }
    };
    let default_name = format!("{}.{}", slug(&active.title), extension);
    let relative = Path::new(requested.unwrap_or(&default_name));
    let output = safe_workspace_output(root, relative)?;
    if output.exists() {
        bail!("refusing to overwrite existing output {}", output.display());
    }

    let report = report_from_view(active)?;
    let rendered = match format {
        SaveFormat::Markdown => loremesh_report::render_markdown(&report),
        SaveFormat::MarkdownMermaid => {
            let diagram = active
                .mermaid
                .as_deref()
                .context("the active view has no Mermaid diagram")?;
            format!(
                "{}\n## Diagram\n\n```mermaid\n{}\n```\n",
                loremesh_report::render_markdown(&report),
                diagram.trim_end()
            )
        }
        SaveFormat::MarkdownD2 => {
            let diagram = active
                .d2
                .as_deref()
                .context("the active view has no D2 diagram")?;
            format!(
                "{}\n## Diagram\n\n```d2\n{}\n```\n",
                loremesh_report::render_markdown(&report),
                diagram.trim_end()
            )
        }
        SaveFormat::Csv => loremesh_report::render_csv(&report)?,
        SaveFormat::Html => loremesh_report::render_html(&report),
        SaveFormat::Png => bail!("PNG export requires a configured local renderer"),
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    let temporary = output.with_extension(format!("{extension}.tmp"));
    fs::write(&temporary, rendered)
        .with_context(|| format!("could not write temporary output {}", temporary.display()))?;
    fs::rename(&temporary, &output)
        .with_context(|| format!("could not finalize output {}", output.display()))?;
    Ok(format!("Saved {}", relative.display()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use loremesh_tui::ViewTable;

    fn trace_view() -> ViewContent {
        ViewContent {
            title: "Evidence trace".into(),
            paragraphs: vec!["Finding to immutable snapshot.".into()],
            table: Some(ViewTable {
                columns: vec!["From".into(), "To".into()],
                rows: vec![vec!["finding".into(), "evidence".into()]],
            }),
            chart: None,
            mermaid: Some("flowchart LR\n  finding --> evidence".into()),
            d2: Some("finding -> evidence".into()),
        }
    }

    #[test]
    fn active_view_save_is_structured_and_never_overwrites() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let message = save_active_view(
            directory.path(),
            &trace_view(),
            SaveFormat::MarkdownMermaid,
            Some("exports/trace.md"),
        )
        .expect("save active view");
        assert_eq!(message, "Saved exports/trace.md");
        let saved =
            fs::read_to_string(directory.path().join("exports/trace.md")).expect("read saved view");
        assert!(saved.contains("```mermaid"));
        assert!(saved.contains("finding --> evidence"));
        assert!(save_active_view(
            directory.path(),
            &trace_view(),
            SaveFormat::Markdown,
            Some("exports/trace.md")
        )
        .is_err());
    }

    #[test]
    fn active_view_save_rejects_traversal_and_unconfigured_png() {
        let directory = tempfile::tempdir().expect("temporary directory");
        assert!(save_active_view(
            directory.path(),
            &trace_view(),
            SaveFormat::Csv,
            Some("../outside.csv")
        )
        .is_err());
        assert!(save_active_view(directory.path(), &trace_view(), SaveFormat::Png, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn active_view_save_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        symlink(outside.path(), workspace.path().join("escape")).expect("symlink fixture");
        assert!(save_active_view(
            workspace.path(),
            &trace_view(),
            SaveFormat::Markdown,
            Some("escape/view.md")
        )
        .is_err());
    }
}
