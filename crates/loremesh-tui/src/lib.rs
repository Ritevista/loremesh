//! Reusable interactive terminal shell for `LoreMesh` workbench views.
#![forbid(unsafe_code)]

pub mod browser;
pub mod chart;
pub mod grid;
pub mod markdown;
pub mod theme;

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use loremesh_core::{Artifact, Finding, Trace};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table,
    Wrap,
};
use ratatui::Terminal;
use thiserror::Error;

const MAX_HISTORY: usize = 100;
const MAX_MESSAGES: usize = 200;

/// Terminal lifecycle failure.
#[derive(Debug, Error)]
#[error("terminal operation failed: {0}")]
pub struct TuiError(#[from] io::Error);

/// A rectangular table associated with active view content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Structured content displayed by the shell and passed to save handlers.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewContent {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub table: Option<ViewTable>,
    pub chart: Option<chart::ChartModel>,
    pub mermaid: Option<String>,
    pub d2: Option<String>,
}

/// Pure presentation data projected from `LoreMesh` domain state.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardView {
    pub workspace_name: String,
    pub summary: ViewContent,
    pub artifacts: ViewContent,
    pub findings: ViewContent,
    pub trace: ViewContent,
}

impl DashboardView {
    /// Projects canonical domain state into generic workbench content.
    // Keeping this projection together makes the four views visibly consistent.
    #[allow(clippy::too_many_lines)]
    pub fn from_domain(
        workspace_name: &str,
        artifacts: &[Artifact],
        findings: &[Finding],
        trace: Option<&Trace>,
    ) -> Self {
        let artifact_rows = artifacts
            .iter()
            .map(|artifact| {
                vec![
                    artifact.name.clone(),
                    artifact.id.to_string(),
                    artifact.byte_len.to_string(),
                ]
            })
            .collect::<Vec<_>>();
        let finding_rows = findings
            .iter()
            .map(|finding| {
                vec![
                    finding.title.clone(),
                    format!("{:?}", finding.status),
                    format!("{:?}", finding.scope),
                ]
            })
            .collect::<Vec<_>>();
        let selected = findings.first().map_or_else(
            || "No finding is selected.".into(),
            |finding| {
                format!(
                    "{}\nStatus: {:?}\nScope: {:?}\nClaims: {}",
                    finding.title,
                    finding.status,
                    finding.scope,
                    finding.claims.len()
                )
            },
        );
        let trace_nodes = trace
            .map(|value| value.nodes().collect::<Vec<_>>())
            .unwrap_or_default();
        let trace_rows = trace_nodes
            .iter()
            .map(|node| vec![node.label.clone(), node.id.to_string()])
            .collect::<Vec<_>>();
        let mermaid = (!trace_nodes.is_empty()).then(|| {
            let mut value = String::from("flowchart TD\n");
            for pair in trace_nodes.windows(2) {
                let _ = writeln!(
                    value,
                    "  {}[\"{}\"] --> {}[\"{}\"]",
                    pair[0].id,
                    pair[0].label.replace('"', "'"),
                    pair[1].id,
                    pair[1].label.replace('"', "'")
                );
            }
            value
        });
        let d2 = (!trace_nodes.is_empty()).then(|| {
            trace_nodes
                .windows(2)
                .map(|pair| {
                    format!(
                        "{}: {}\n{}: {}\n{} -> {}",
                        pair[0].id,
                        pair[0].label,
                        pair[1].id,
                        pair[1].label,
                        pair[0].id,
                        pair[1].id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        });
        Self {
            workspace_name: workspace_name.into(),
            summary: ViewContent {
                title: "Investigation".into(),
                paragraphs: vec![
                    format!("Workspace: {workspace_name}"),
                    format!(
                        "{} artifacts · {} findings",
                        artifacts.len(),
                        findings.len()
                    ),
                    selected,
                ],
                table: None,
                chart: None,
                mermaid: None,
                d2: None,
            },
            artifacts: ViewContent {
                title: "Artifacts".into(),
                paragraphs: vec!["Imported immutable source material.".into()],
                table: Some(ViewTable {
                    columns: vec!["Name".into(), "Artifact ID".into(), "Bytes".into()],
                    rows: artifact_rows,
                }),
                chart: None,
                mermaid: None,
                d2: None,
            },
            findings: ViewContent {
                title: "Findings".into(),
                paragraphs: vec!["Evidence-backed investigation findings.".into()],
                table: Some(ViewTable {
                    columns: vec!["Title".into(), "Status".into(), "Scope".into()],
                    rows: finding_rows,
                }),
                chart: None,
                mermaid: None,
                d2: None,
            },
            trace: ViewContent {
                title: "Evidence path / lineage".into(),
                paragraphs: vec![trace_nodes
                    .iter()
                    .map(|node| node.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ")],
                table: Some(ViewTable {
                    columns: vec!["Node".into(), "ID".into()],
                    rows: trace_rows,
                }),
                chart: None,
                mermaid,
                d2,
            },
        }
    }

    fn content(&self, kind: ViewKind) -> &ViewContent {
        match kind {
            ViewKind::Summary | ViewKind::Custom => &self.summary,
            ViewKind::Artifacts => &self.artifacts,
            ViewKind::Findings => &self.findings,
            ViewKind::Trace => &self.trace,
        }
    }
}

/// Focusable shell region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Primary,
    Timeline,
    Input,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Primary => Self::Timeline,
            Self::Timeline => Self::Input,
            Self::Input => Self::Primary,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Primary => Self::Input,
            Self::Timeline => Self::Primary,
            Self::Input => Self::Timeline,
        }
    }
}

/// Generic view selected by a navigation command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Summary,
    Artifacts,
    Findings,
    Trace,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableCommand {
    Load(String),
    Refresh,
    Save(String),
    Sort { column: String, descending: bool },
    Filter { column: String, value: String },
    Search(String),
    Columns(Vec<String>),
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCommand {
    Enter,
    Status,
    Enable,
    Disable,
    Run(String),
    Input(String),
    Interrupt,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    LoreMesh,
    Shell,
    Find,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserCommand {
    Browse(Option<String>),
    Open(String),
    Search(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoKind {
    Table,
    Chart,
    Markdown,
    Code,
    Shell,
}

/// Supported structured save format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    Markdown,
    MarkdownMermaid,
    MarkdownD2,
    Csv,
    Html,
    Png,
}

/// Investigation workflow command parsed without storage access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvestigationCommand {
    New { title: String, organization: bool },
    List,
    Open(String),
    AddCurrent,
    Add { kind: String, id: String },
    Remove { kind: String, id: String },
    Show,
    Trace,
    Note(String),
    Status(String),
    Save,
    ExportHtml { output: String },
}

/// Typed slash command with no shell interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Help,
    View(ViewKind),
    Services,
    Model,
    Context,
    Compact,
    Clear,
    Save {
        format: SaveFormat,
        output: Option<String>,
    },
    Table(TableCommand),
    Chart {
        kind: chart::ChartKind,
        label_column: String,
        value_column: String,
    },
    Shell(ShellCommand),
    Browser(BrowserCommand),
    KnowledgeSearch(String),
    Investigation(InvestigationCommand),
    Demo(DemoKind),
    Quit,
}

/// Slash-command parsing failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("command error: {0}")]
pub struct CommandError(String);

/// Parses one slash command without invoking a shell.
pub fn parse_command(input: &str) -> Result<SlashCommand, CommandError> {
    let mut parts = input.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| CommandError("command must not be blank".into()))?;
    match name {
        "/help" => no_args(parts, SlashCommand::Help),
        "/artifacts" => no_args(parts, SlashCommand::View(ViewKind::Artifacts)),
        "/findings" => no_args(parts, SlashCommand::View(ViewKind::Findings)),
        "/trace" => no_args(parts, SlashCommand::View(ViewKind::Trace)),
        "/services" => no_args(parts, SlashCommand::Services),
        "/model" => no_args(parts, SlashCommand::Model),
        "/context" => no_args(parts, SlashCommand::Context),
        "/compact" => no_args(parts, SlashCommand::Compact),
        "/clear" => no_args(parts, SlashCommand::Clear),
        "/demo" => parse_demo(parts),
        "/table" => parse_table(parts),
        "/chart" => parse_chart(parts),
        "/shell" => parse_shell(parts),
        "/browse" => {
            let path = parts.collect::<Vec<_>>().join(" ");
            Ok(SlashCommand::Browser(BrowserCommand::Browse(
                (!path.is_empty()).then_some(path),
            )))
        }
        "/open" => one_rest(parts, "usage: /open <path>")
            .map(|path| SlashCommand::Browser(BrowserCommand::Open(path))),
        "/search" => one_rest(parts, "usage: /search <text>").map(SlashCommand::KnowledgeSearch),
        "/investigation" => parse_investigation(parts),
        "/find" => one_rest(parts, "usage: /find <text>")
            .map(|query| SlashCommand::Browser(BrowserCommand::Search(query))),
        "/quit" | "/exit" => no_args(parts, SlashCommand::Quit),
        "/save" | "/export" => parse_save(parts),
        _ => Err(CommandError(format!("unknown command '{name}'; use /help"))),
    }
}

#[allow(clippy::too_many_lines)]
fn parse_investigation<'a>(
    mut parts: impl Iterator<Item = &'a str>,
) -> Result<SlashCommand, CommandError> {
    let action = parts.next().ok_or_else(|| {
        CommandError(
            "usage: /investigation <new|list|open|add|remove|show|trace|note|status|save|export>"
                .into(),
        )
    })?;
    let command = match action {
        "new" => {
            let values = parts.collect::<Vec<_>>();
            let (organization, title_values) = if values.starts_with(&["--scope", "organization"]) {
                (true, &values[2..])
            } else if values.starts_with(&["--scope", "personal"]) {
                (false, &values[2..])
            } else {
                (false, values.as_slice())
            };
            let title = title_values.join(" ").trim_matches('"').to_owned();
            if title.is_empty() {
                return Err(CommandError(
                    "usage: /investigation new [--scope personal|organization] <title>".into(),
                ));
            }
            InvestigationCommand::New {
                title,
                organization,
            }
        }
        "list" => {
            return no_args(
                parts,
                SlashCommand::Investigation(InvestigationCommand::List),
            )
        }
        "open" => InvestigationCommand::Open(one_rest(parts, "usage: /investigation open <id>")?),
        "add" => match parts.next() {
            Some("current") => {
                return no_args(
                    parts,
                    SlashCommand::Investigation(InvestigationCommand::AddCurrent),
                )
            }
            Some(kind) => InvestigationCommand::Add {
                kind: kind.into(),
                id: parts
                    .next()
                    .ok_or_else(|| CommandError("usage: /investigation add <kind> <id>".into()))?
                    .into(),
            },
            None => {
                return Err(CommandError(
                    "usage: /investigation add <current|kind id>".into(),
                ))
            }
        },
        "remove" => InvestigationCommand::Remove {
            kind: parts
                .next()
                .ok_or_else(|| CommandError("usage: /investigation remove <kind> <id>".into()))?
                .into(),
            id: parts
                .next()
                .ok_or_else(|| CommandError("usage: /investigation remove <kind> <id>".into()))?
                .into(),
        },
        "show" => {
            return no_args(
                parts,
                SlashCommand::Investigation(InvestigationCommand::Show),
            )
        }
        "trace" => {
            return no_args(
                parts,
                SlashCommand::Investigation(InvestigationCommand::Trace),
            )
        }
        "note" => InvestigationCommand::Note(one_rest(parts, "usage: /investigation note <text>")?),
        "status" => {
            InvestigationCommand::Status(one_rest(parts, "usage: /investigation status <status>")?)
        }
        "save" => {
            return no_args(
                parts,
                SlashCommand::Investigation(InvestigationCommand::Save),
            )
        }
        "export" => {
            let values = parts.collect::<Vec<_>>();
            if values.len() != 4
                || values[0] != "--format"
                || values[1] != "html"
                || values[2] != "--output"
            {
                return Err(CommandError(
                    "usage: /investigation export --format html --output <path>".into(),
                ));
            }
            InvestigationCommand::ExportHtml {
                output: values[3].into(),
            }
        }
        _ => {
            return Err(CommandError(format!(
                "unknown investigation action '{action}'"
            )))
        }
    };
    Ok(SlashCommand::Investigation(command))
}

fn parse_demo<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<SlashCommand, CommandError> {
    let kind = match parts.next() {
        Some("table") => DemoKind::Table,
        Some("chart") => DemoKind::Chart,
        Some("markdown") => DemoKind::Markdown,
        Some("code") => DemoKind::Code,
        Some("shell") => DemoKind::Shell,
        _ => {
            return Err(CommandError(
                "usage: /demo <table|chart|markdown|code|shell>".into(),
            ))
        }
    };
    no_args(parts, SlashCommand::Demo(kind))
}

fn one_rest<'a>(parts: impl Iterator<Item = &'a str>, usage: &str) -> Result<String, CommandError> {
    let value = parts.collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        Err(CommandError(usage.into()))
    } else {
        Ok(value)
    }
}

fn parse_table<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<SlashCommand, CommandError> {
    let action = parts.next().ok_or_else(|| {
        CommandError("usage: /table <load|refresh|save|sort|filter|search|columns|reset>".into())
    })?;
    let command = match action {
        "load" => TableCommand::Load(one_rest(parts, "usage: /table load <path>")?),
        "save" => TableCommand::Save(one_rest(parts, "usage: /table save <path>")?),
        "refresh" => return no_args(parts, SlashCommand::Table(TableCommand::Refresh)),
        "reset" => return no_args(parts, SlashCommand::Table(TableCommand::Reset)),
        "search" => TableCommand::Search(parts.collect::<Vec<_>>().join(" ")),
        "columns" => {
            let names = one_rest(parts, "usage: /table columns <name,...>")?
                .split(',')
                .map(|name| name.trim().to_owned())
                .collect();
            TableCommand::Columns(names)
        }
        "sort" => {
            let column = parts
                .next()
                .ok_or_else(|| CommandError("usage: /table sort <column> <asc|desc>".into()))?;
            let descending = match parts.next() {
                Some("asc") => false,
                Some("desc") => true,
                _ => {
                    return Err(CommandError(
                        "usage: /table sort <column> <asc|desc>".into(),
                    ))
                }
            };
            if parts.next().is_some() {
                return Err(CommandError(
                    "usage: /table sort <column> <asc|desc>".into(),
                ));
            }
            TableCommand::Sort {
                column: column.into(),
                descending,
            }
        }
        "filter" => {
            let column = parts
                .next()
                .ok_or_else(|| CommandError("usage: /table filter <column> <text>".into()))?;
            let value = parts.collect::<Vec<_>>().join(" ");
            TableCommand::Filter {
                column: column.into(),
                value,
            }
        }
        _ => return Err(CommandError(format!("unknown table action '{action}'"))),
    };
    Ok(SlashCommand::Table(command))
}

fn parse_chart<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<SlashCommand, CommandError> {
    let kind = match parts.next() {
        Some("bar") => chart::ChartKind::Bar,
        Some("hbar") => chart::ChartKind::HorizontalBar,
        Some("line") => chart::ChartKind::Line,
        Some("pie") => chart::ChartKind::Pie,
        _ => {
            return Err(CommandError(
                "usage: /chart <bar|hbar|line|pie> <label-column> <value-column>".into(),
            ))
        }
    };
    let label_column = parts
        .next()
        .ok_or_else(|| CommandError("chart label column is required".into()))?;
    let value_column = parts
        .next()
        .ok_or_else(|| CommandError("chart value column is required".into()))?;
    if parts.next().is_some() {
        return Err(CommandError("chart accepts exactly two columns".into()));
    }
    Ok(SlashCommand::Chart {
        kind,
        label_column: label_column.into(),
        value_column: value_column.into(),
    })
}

fn parse_shell<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<SlashCommand, CommandError> {
    match parts.next() {
        None => Ok(SlashCommand::Shell(ShellCommand::Enter)),
        Some("status") => no_args(parts, SlashCommand::Shell(ShellCommand::Status)),
        Some("enable") => no_args(parts, SlashCommand::Shell(ShellCommand::Enable)),
        Some("disable") => no_args(parts, SlashCommand::Shell(ShellCommand::Disable)),
        Some("run") => one_rest(parts, "usage: /shell run <command>")
            .map(|command| SlashCommand::Shell(ShellCommand::Run(command))),
        _ => Err(CommandError(
            "usage: /shell <status|enable|disable|run>".into(),
        )),
    }
}

fn no_args<'a>(
    mut parts: impl Iterator<Item = &'a str>,
    command: SlashCommand,
) -> Result<SlashCommand, CommandError> {
    if parts.next().is_some() {
        Err(CommandError("command does not accept arguments".into()))
    } else {
        Ok(command)
    }
}

fn parse_save<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<SlashCommand, CommandError> {
    if parts.next() != Some("current") {
        return Err(CommandError(
            "usage: /save current --format <md|markdown-mermaid|markdown-d2|csv|html|png> [--output <path>]"
                .into(),
        ));
    }
    let mut format = None;
    let mut output = None;
    while let Some(option) = parts.next() {
        match option {
            "--format" => {
                let value = parts
                    .next()
                    .ok_or_else(|| CommandError("--format requires a value".into()))?;
                format = Some(match value {
                    "md" | "markdown" => SaveFormat::Markdown,
                    "markdown-mermaid" | "mmd" => SaveFormat::MarkdownMermaid,
                    "markdown-d2" | "d2" => SaveFormat::MarkdownD2,
                    "csv" => SaveFormat::Csv,
                    "html" => SaveFormat::Html,
                    "png" => SaveFormat::Png,
                    _ => return Err(CommandError(format!("unsupported save format '{value}'"))),
                });
            }
            "--output" => {
                output = Some(
                    parts
                        .next()
                        .ok_or_else(|| CommandError("--output requires a path".into()))?
                        .to_owned(),
                );
            }
            _ => return Err(CommandError(format!("unknown save option '{option}'"))),
        }
    }
    Ok(SlashCommand::Save {
        format: format.ok_or_else(|| CommandError("--format is required".into()))?,
        output,
    })
}

/// Content-safe result from application command handling.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResponse {
    pub message: String,
    pub content: Option<ViewContent>,
    pub input_mode: Option<InputMode>,
}

/// Application boundary for commands that require services or filesystem I/O.
pub trait CommandHandler {
    fn execute(&mut self, command: &SlashCommand, active: &ViewContent) -> CommandResponse;

    fn activate(&mut self, _active: &ViewContent, row: usize) -> CommandResponse {
        CommandResponse {
            message: format!("Selected row {}", row.saturating_add(1)),
            content: None,
            input_mode: None,
        }
    }

    fn poll(&mut self) -> Option<CommandResponse> {
        None
    }

    fn resize(&mut self, _rows: u16, _cols: u16) {}
}

/// Pure interactive shell state.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellState {
    pub focus: Focus,
    pub input: String,
    pub active: ViewKind,
    pub messages: VecDeque<String>,
    history: VecDeque<String>,
    history_cursor: Option<usize>,
    should_quit: bool,
    custom: Option<ViewContent>,
    back_stack: Vec<ViewContent>,
    primary_scroll: u16,
    timeline_scroll: u16,
    selected_row: usize,
    input_mode: InputMode,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            focus: Focus::Primary,
            input: String::new(),
            active: ViewKind::Summary,
            messages: VecDeque::from([String::from(
                "Ready. Press / for commands or Tab to change focus.",
            )]),
            history: VecDeque::new(),
            history_cursor: None,
            should_quit: false,
            custom: None,
            back_stack: Vec::new(),
            primary_scroll: 0,
            timeline_scroll: 0,
            selected_row: 0,
            input_mode: InputMode::LoreMesh,
        }
    }
}

impl ShellState {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Applies one keyboard event and optionally executes a complete command.
    pub fn handle_key<H: CommandHandler>(
        &mut self,
        key: KeyEvent,
        view: &DashboardView,
        handler: &mut H,
    ) {
        match key.code {
            KeyCode::Char('c')
                if self.input_mode == InputMode::Shell
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.apply_response(handler.execute(
                    &SlashCommand::Shell(ShellCommand::Interrupt),
                    self.content(view),
                ));
            }
            KeyCode::Char('d')
                if self.input_mode == InputMode::Shell
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.apply_response(
                    handler.execute(&SlashCommand::Shell(ShellCommand::Exit), self.content(view)),
                );
            }
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus = self.focus.previous();
            }
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::Char('/') if self.focus != Focus::Input => {
                self.focus = Focus::Input;
                self.input = "/".into();
            }
            KeyCode::Char('q') if self.focus != Focus::Input => self.should_quit = true,
            KeyCode::Char('b') if self.focus == Focus::Primary => self.go_back(),
            KeyCode::Char('f')
                if self.focus == Focus::Primary
                    && self.content(view).table.is_none()
                    && self.content(view).chart.is_none()
                    && is_open_document(self.content(view)) =>
            {
                self.begin_find();
            }
            KeyCode::Esc if self.focus == Focus::Input => {
                self.leave_input();
            }
            KeyCode::Esc => self.focus = Focus::Primary,
            KeyCode::PageUp => {
                self.scroll_focused(-10, view);
            }
            KeyCode::PageDown => {
                self.scroll_focused(10, view);
            }
            KeyCode::Home if self.focus == Focus::Primary => {
                self.primary_scroll = 0;
                self.selected_row = 0;
            }
            KeyCode::Home if self.focus == Focus::Timeline => self.timeline_scroll = 0,
            KeyCode::End if self.focus != Focus::Input => {
                if self.focus == Focus::Primary {
                    self.primary_scroll = self.maximum_primary_scroll(view);
                    self.selected_row = self.table_last_row(view);
                } else {
                    self.timeline_scroll = self.maximum_timeline_scroll();
                }
            }
            KeyCode::Up if self.focus == Focus::Primary && self.content(view).table.is_some() => {
                self.selected_row = self.selected_row.saturating_sub(1);
                let selected = u16::try_from(self.selected_row).unwrap_or(u16::MAX);
                if selected < self.primary_scroll {
                    self.primary_scroll = selected;
                }
            }
            KeyCode::Down if self.focus == Focus::Primary && self.content(view).table.is_some() => {
                self.selected_row = self
                    .selected_row
                    .saturating_add(1)
                    .min(self.table_last_row(view));
                let selected = u16::try_from(self.selected_row).unwrap_or(u16::MAX);
                if selected >= self.primary_scroll.saturating_add(10) {
                    self.primary_scroll = selected.saturating_sub(9);
                }
            }
            KeyCode::Enter
                if self.focus == Focus::Primary && self.content(view).table.is_some() =>
            {
                self.activate_selection(view, handler);
            }
            KeyCode::Char('e') if self.focus == Focus::Primary => {
                self.focus = Focus::Input;
                self.input = "/save current --format ".into();
            }
            KeyCode::Enter if self.focus == Focus::Input => self.submit(view, handler),
            KeyCode::Backspace if self.focus == Focus::Input => {
                self.input.pop();
            }
            KeyCode::Char(character)
                if self.focus == Focus::Input && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.input.push(character);
            }
            KeyCode::Up if self.focus == Focus::Input => self.history_previous(),
            KeyCode::Down if self.focus == Focus::Input => self.history_next(),
            _ => {}
        }
    }

    fn submit<H: CommandHandler>(&mut self, view: &DashboardView, handler: &mut H) {
        let input = self.input.trim().to_owned();
        self.input.clear();
        self.history_cursor = None;
        if input.is_empty() {
            return;
        }
        if self.input_mode == InputMode::Find {
            self.input_mode = InputMode::LoreMesh;
            let response = handler.execute(
                &SlashCommand::Browser(BrowserCommand::Search(input)),
                self.content(view),
            );
            self.apply_response(response);
            return;
        }
        if self.input_mode == InputMode::Shell {
            if input == "/quit" {
                self.should_quit = true;
                return;
            }
            let command = if input == "/exit" {
                ShellCommand::Exit
            } else {
                ShellCommand::Input(input)
            };
            let response = handler.execute(&SlashCommand::Shell(command), self.content(view));
            self.apply_response(response);
            return;
        }
        let parsed = parse_command(&input);
        if !matches!(&parsed, Ok(SlashCommand::Shell(ShellCommand::Run(_)))) {
            self.history.push_back(input);
            while self.history.len() > MAX_HISTORY {
                self.history.pop_front();
            }
        }
        match parsed {
            Ok(SlashCommand::Help) => {
                self.show_content(help_content());
                self.push_message("Opened command reference");
            }
            Ok(SlashCommand::View(kind)) => {
                self.active = kind;
                self.focus = Focus::Primary;
                self.primary_scroll = 0;
                self.selected_row = 0;
                self.push_message(format!("Opened {}", view.content(kind).title));
            }
            Ok(SlashCommand::Clear) => {
                self.messages.clear();
                self.focus = Focus::Primary;
            }
            Ok(SlashCommand::Quit) => self.should_quit = true,
            Ok(command) => {
                let response = handler.execute(&command, self.content(view));
                self.apply_response(response);
            }
            Err(error) => {
                let message = error.to_string();
                self.show_content(text_content("Command error", &message));
                self.push_message(message);
            }
        }
    }

    fn apply_response(&mut self, response: CommandResponse) {
        if !response.message.is_empty() {
            self.push_message(response.message);
        }
        if let Some(mode) = response.input_mode {
            self.input_mode = mode;
        }
        self.focus = if self.input_mode == InputMode::Shell {
            Focus::Input
        } else {
            Focus::Primary
        };
        if let Some(content) = response.content {
            if self.input_mode == InputMode::Shell {
                self.custom = Some(content);
                self.active = ViewKind::Custom;
                self.focus = Focus::Input;
            } else {
                self.show_content(content);
            }
        }
    }

    fn show_content(&mut self, content: ViewContent) {
        if let Some(current) = self.custom.take() {
            self.back_stack.push(current);
        }
        self.custom = Some(content);
        self.active = ViewKind::Custom;
        self.focus = Focus::Primary;
        self.primary_scroll = 0;
        self.selected_row = 0;
    }

    fn go_back(&mut self) {
        if let Some(previous) = self.back_stack.pop() {
            self.custom = Some(previous);
            self.active = ViewKind::Custom;
            self.focus = Focus::Primary;
            self.primary_scroll = 0;
            self.selected_row = 0;
            self.push_message("Returned to previous view");
        } else {
            self.push_message("No previous view");
        }
    }

    fn begin_find(&mut self) {
        self.input_mode = InputMode::Find;
        self.input.clear();
        self.focus = Focus::Input;
    }

    fn activate_selection<H: CommandHandler>(&mut self, view: &DashboardView, handler: &mut H) {
        let response = handler.activate(self.content(view), self.selected_row);
        self.apply_response(response);
    }

    fn leave_input(&mut self) {
        self.input.clear();
        if self.input_mode == InputMode::Find {
            self.input_mode = InputMode::LoreMesh;
        }
        self.focus = Focus::Primary;
        self.history_cursor = None;
    }

    fn maximum_primary_scroll(&self, view: &DashboardView) -> u16 {
        let content = self.content(view);
        let lines = content.table.as_ref().map_or_else(
            || {
                content
                    .paragraphs
                    .iter()
                    .map(|paragraph| paragraph.lines().count().max(1))
                    .sum::<usize>()
                    .saturating_add(self.messages.len())
            },
            |table| table.rows.len(),
        );
        u16::try_from(lines.saturating_sub(1)).map_or(u16::MAX, std::convert::identity)
    }

    fn maximum_timeline_scroll(&self) -> u16 {
        u16::try_from(self.messages.len().saturating_sub(1)).unwrap_or(u16::MAX)
    }

    fn table_last_row(&self, view: &DashboardView) -> usize {
        self.content(view)
            .table
            .as_ref()
            .map_or(0, |table| table.rows.len().saturating_sub(1))
    }

    fn scroll_focused(&mut self, delta: i16, view: &DashboardView) {
        let (current, maximum) = if self.focus == Focus::Timeline {
            (self.timeline_scroll, self.maximum_timeline_scroll())
        } else {
            (self.primary_scroll, self.maximum_primary_scroll(view))
        };
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(maximum)
        };
        if self.focus == Focus::Timeline {
            self.timeline_scroll = next;
        } else {
            self.primary_scroll = next;
        }
    }

    fn content<'a>(&'a self, view: &'a DashboardView) -> &'a ViewContent {
        if self.active == ViewKind::Custom {
            self.custom.as_ref().unwrap_or(&view.summary)
        } else {
            view.content(self.active)
        }
    }

    fn push_message(&mut self, message: impl Into<String>) {
        self.messages.push_back(message.into());
        while self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.timeline_scroll = self.maximum_timeline_scroll();
    }

    fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let index = self
            .history_cursor
            .map_or(self.history.len() - 1, |value| value.saturating_sub(1));
        self.history_cursor = Some(index);
        if let Some(value) = self.history.get(index) {
            self.input.clone_from(value);
        }
    }

    fn history_next(&mut self) {
        let Some(index) = self.history_cursor else {
            return;
        };
        if index + 1 >= self.history.len() {
            self.history_cursor = None;
            self.input.clear();
        } else {
            self.history_cursor = Some(index + 1);
            if let Some(value) = self.history.get(index + 1) {
                self.input.clone_from(value);
            }
        }
    }
}

fn text_content(title: &str, text: &str) -> ViewContent {
    ViewContent {
        title: title.into(),
        paragraphs: vec![text.into()],
        table: None,
        chart: None,
        mermaid: None,
        d2: None,
    }
}

fn base_help_content() -> ViewContent {
    text_content(
        "LoreMesh command reference",
        "LOREMESH COMMAND REFERENCE\n\nNAVIGATION\n/help\n  Show this complete reference.\n/artifacts\n  Show imported artifacts.\n/findings\n  Show findings.\n/trace\n  Show evidence lineage.\n/search <text>\n  Search canonical knowledge; select with Up/Down and open with Enter.\n\nDEMONSTRATIONS\n/demo table\n/demo chart\n/demo markdown\n/demo code\n/demo shell\n  Open deterministic capability previews; no files, network, model, or shell execution required.\n\nTABLES\n/table load <workspace-relative.csv>\n/table refresh\n/table save <workspace-relative.csv>\n/table sort <column> <asc|desc>\n/table filter <column> <text>\n/table search <text>\n/table columns <column,...>\n/table reset\n\nCHARTS\n/chart <bar|hbar|line|pie> <label-column> <value-column>\n  Requires a loaded table. Example: /chart hbar name duration\n\nFILES AND MARKDOWN\n/browse [workspace-relative-directory]\n/open <workspace-relative-file>\n/find <text>\n  Search within the currently opened file.\n\nLOCAL SHELL\n/shell\n  Start a persistent shell in the workspace. Type commands normally in the bottom composer.\n/exit or Ctrl-D\n  Close the shell and return to LoreMesh command mode.\nCtrl-C\n  Interrupt the current shell command. PgUp/PgDn and Home/End scroll its bounded timeline output.\n  The shell has your OS permissions and may access files or networks. LoreMesh does not retain its command history.\n\nREPORTS\n/save current --format <md|markdown-mermaid|markdown-d2|csv|html|png> [--output <path>]\n/export current --format <format> [--output <path>]\n\nSERVICES\n/services\n/model\n/context\n/compact\n/clear\n\nEXIT\n/quit\n/exit\n  Outside input, q exits. In shell mode /exit returns to LoreMesh and /quit exits the app. Esc changes focus and never exits.",
    )
}

fn help_content() -> ViewContent {
    let mut content = base_help_content();
    content.paragraphs.push(
        "INVESTIGATIONS\n/investigation new [--scope personal|organization] <title>\n/investigation list\n/investigation open <id>\n/investigation add <current|kind id>\n/investigation remove <kind> <id>\n/investigation show\n/investigation trace\n/investigation note <text>\n/investigation status <draft|in-review|reviewed|archived>\n/investigation save\n/investigation export --format html --output <path>"
            .into(),
    );
    content
}

fn is_open_document(content: &ViewContent) -> bool {
    content.paragraphs.first().is_some_and(|paragraph| {
        paragraph.starts_with("Path: ") || paragraph.starts_with("Artifact: ")
    })
}

/// Runs the interactive shell until a quit command or key is received.
pub fn run<H: CommandHandler>(view: &DashboardView, handler: &mut H) -> Result<(), TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = ShellState::default();
    let result = event_loop(&mut terminal, view, &mut state, handler);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop<H: CommandHandler>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    view: &DashboardView,
    state: &mut ShellState,
    handler: &mut H,
) -> Result<(), TuiError> {
    while !state.should_quit() {
        while let Some(response) = handler.poll() {
            state.apply_response(response);
        }
        terminal.draw(|frame| {
            let area = frame.area();
            handler.resize(area.height.saturating_sub(7), area.width);
            draw(frame, view, state);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                state.handle_key(key, view, handler);
            }
        }
    }
    Ok(())
}

fn focus_block(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        theme::focused()
    } else {
        Style::default().fg(theme::MUTED)
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style)
}

fn draw(frame: &mut ratatui::Frame<'_>, view: &DashboardView, state: &ShellState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("LoreMesh", theme::header()),
            Span::raw(format!(
                " · KB: {} · Workspace: {} · offline · AI: off",
                view.workspace_name, view.workspace_name
            )),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        rows[0],
    );
    draw_primary(frame, rows[1], view, state);
    draw_lineage(frame, rows[2], view, state);
    draw_history(frame, rows[3], state);
    let input_style = if state.focus == Focus::Input {
        Style::default()
            .fg(theme::WARNING)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::TEXT)
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{} {}",
            if state.input_mode == InputMode::Shell {
                "$"
            } else if state.input_mode == InputMode::Find {
                "?"
            } else {
                ">"
            },
            state.input
        ))
        .style(input_style)
        .block(focus_block(
            if state.input_mode == InputMode::Shell {
                "Shell"
            } else if state.input_mode == InputMode::Find {
                "Find in document"
            } else {
                "Command"
            },
            state.focus == Focus::Input,
        )),
        rows[4],
    );
    frame.render_widget(
        Paragraph::new(contextual_shortcuts(state, view)).style(Style::default().fg(theme::MUTED)),
        rows[5],
    );
}

fn draw_primary(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &DashboardView,
    state: &ShellState,
) {
    let active = state.content(view);
    if let Some(chart) = &active.chart {
        draw_chart(frame, area, chart, state.focus == Focus::Primary);
    } else if let Some(table) = &active.table {
        let widths = (0..table.columns.len())
            .map(|_| Constraint::Fill(1))
            .collect::<Vec<_>>();
        let rows =
            table
                .rows
                .iter()
                .skip(usize::from(state.primary_scroll))
                .enumerate()
                .map(|(index, row)| {
                    let absolute = index.saturating_add(usize::from(state.primary_scroll));
                    let style = if absolute == state.selected_row {
                        theme::selected()
                    } else if absolute % 2 == 0 {
                        Style::default()
                    } else {
                        Style::default().bg(theme::SURFACE_ALT)
                    };
                    Row::new(row.iter().map(|value| {
                        Cell::from(value.clone()).style(theme::value(value).patch(style))
                    }))
                });
        let title = format!(
            "{} · {} rows × {} columns{}",
            active.title,
            table.rows.len(),
            table.columns.len(),
            if table.rows.is_empty() {
                " · no results"
            } else {
                ""
            }
        );
        frame.render_widget(
            Table::new(rows, widths)
                .header(
                    Row::new(table.columns.clone())
                        .style(theme::header())
                        .bottom_margin(1),
                )
                .column_spacing(1)
                .block(focus_block(&title, state.focus == Focus::Primary)),
            area,
        );
    } else {
        frame.render_widget(
            Paragraph::new(active.paragraphs.join("\n\n"))
                .style(Style::default().fg(theme::TEXT))
                .scroll((state.primary_scroll, 0))
                .wrap(Wrap { trim: false })
                .block(focus_block(&active.title, state.focus == Focus::Primary)),
            area,
        );
    }
}

fn draw_lineage(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &DashboardView,
    state: &ShellState,
) {
    let active = state.content(view);
    let lineage = if let Some(table) = &active.table {
        table.rows.get(state.selected_row).map_or_else(
            || "No row selected".into(),
            |row| {
                format!(
                    "Selected row {} · {}",
                    state.selected_row + 1,
                    row.join(" → ")
                )
            },
        )
    } else {
        format!(
            "Active view → {} · source lineage available when evidence is selected",
            active.title
        )
    };
    frame.render_widget(
        Paragraph::new(lineage)
            .style(Style::default().fg(theme::SECONDARY))
            .block(Block::default().title("Lineage").borders(Borders::ALL)),
        area,
    );
}

fn draw_history(frame: &mut ratatui::Frame<'_>, area: Rect, state: &ShellState) {
    let history = state
        .messages
        .iter()
        .map(|message| format!("› {message}"))
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(history)
            .style(Style::default().fg(theme::TEXT))
            .scroll((state.timeline_scroll, 0))
            .wrap(Wrap { trim: false })
            .block(focus_block(
                "Investigation timeline",
                state.focus == Focus::Timeline,
            )),
        area,
    );
}

fn contextual_shortcuts(state: &ShellState, view: &DashboardView) -> String {
    if state.focus == Focus::Input {
        return if state.input_mode == InputMode::Find {
            "Enter find · Esc cancel · type plain search text".into()
        } else {
            "Enter run · ↑↓ history · Esc primary · Tab focus · /help".into()
        };
    }
    if state.focus == Focus::Timeline {
        return "PgUp/PgDn Home/End timeline · Tab focus · / command · q quit".into();
    }
    let active = state.content(view);
    if active.table.is_some() {
        "↑↓ select · Enter details · / commands · e export · Tab focus · q quit".into()
    } else if active.chart.is_some() {
        "e export · PgUp/PgDn · Tab focus · / commands · q quit".into()
    } else {
        "PgUp/PgDn scroll · f find · b back · e export · Tab focus · q quit".into()
    }
}

fn draw_chart(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    chart: &chart::ChartModel,
    focused: bool,
) {
    if area.width < 60 || area.height < 10 {
        frame.render_widget(
            Paragraph::new(chart.render_text(usize::from(area.width.saturating_sub(2))))
                .style(Style::default().fg(theme::TEXT))
                .wrap(Wrap { trim: false })
                .block(focus_block(
                    &format!("{} · compact view", chart.title),
                    focused,
                )),
            area,
        );
        return;
    }

    let chart_area = if chart.series.len() > 1 {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        let legend = chart
            .series
            .iter()
            .enumerate()
            .flat_map(|(index, series)| {
                [
                    Span::styled("● ", Style::default().fg(theme::series(index))),
                    Span::styled(
                        format!("{}  ", series.name),
                        Style::default().fg(theme::TEXT),
                    ),
                ]
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(Line::from(legend)), sections[0]);
        sections[1]
    } else {
        area
    };

    match chart.kind {
        chart::ChartKind::Line => draw_line_chart(frame, chart_area, chart, focused),
        chart::ChartKind::Bar | chart::ChartKind::HorizontalBar => {
            draw_bar_chart(frame, chart_area, chart, focused);
        }
        chart::ChartKind::Pie => draw_distribution_chart(frame, chart_area, chart, focused),
    }
}

fn draw_line_chart(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    chart: &chart::ChartModel,
    focused: bool,
) {
    let points = chart
        .series
        .iter()
        .map(|series| {
            series
                .values
                .iter()
                .enumerate()
                .map(|(index, value)| (index_as_f64(index), value.value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let datasets = chart
        .series
        .iter()
        .zip(&points)
        .enumerate()
        .map(|(index, (series, values))| {
            Dataset::default()
                .name(series.name.clone())
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(theme::series(index)))
                .data(values)
        })
        .collect::<Vec<_>>();
    let values = chart
        .series
        .iter()
        .flat_map(|series| series.values.iter().map(|value| value.value))
        .collect::<Vec<_>>();
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = ((maximum - minimum).abs() * 0.1).max(1.0);
    let last = chart.series[0].values.len().saturating_sub(1);
    let middle = last / 2;
    let labels = &chart.series[0].values;
    let x_labels = [0, middle, last]
        .into_iter()
        .map(|index| Line::from(labels[index].label.clone()))
        .collect::<Vec<_>>();
    let y_labels = [
        minimum - padding,
        f64::midpoint(minimum, maximum),
        maximum + padding,
    ]
    .into_iter()
    .map(|value| Line::from(format!("{value:.1}")))
    .collect::<Vec<_>>();
    let widget = Chart::new(datasets)
        .block(focus_block(&chart.title, focused))
        .style(Style::default().fg(theme::TEXT))
        .legend_position(None)
        .x_axis(
            Axis::default()
                .title("Sample")
                .style(Style::default().fg(theme::MUTED))
                .bounds([0.0, index_as_f64(last.max(1))])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("Value")
                .style(Style::default().fg(theme::MUTED))
                .bounds([minimum - padding, maximum + padding])
                .labels(y_labels),
        );
    frame.render_widget(widget, area);
}

fn draw_bar_chart(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    chart: &chart::ChartModel,
    focused: bool,
) {
    let maximum = chart
        .series
        .iter()
        .flat_map(|series| &series.values)
        .map(|value| value.value.abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut widget = BarChart::default()
        .block(focus_block(&chart.title, focused))
        .bar_gap(1)
        .group_gap(2)
        .bar_width(if chart.kind == chart::ChartKind::HorizontalBar {
            1
        } else {
            5
        })
        .direction(if chart.kind == chart::ChartKind::HorizontalBar {
            Direction::Horizontal
        } else {
            Direction::Vertical
        });
    for (category_index, category) in chart.series[0].values.iter().enumerate() {
        let bars = chart
            .series
            .iter()
            .enumerate()
            .map(|(series_index, series)| {
                let value = series.values[category_index].value;
                let ratio = value.abs() / maximum;
                let scaled = (1_u32..=100)
                    .filter(|cell| (f64::from(*cell) / 100.0) <= ratio)
                    .count();
                let scaled = u64::try_from(scaled).map_or(100, std::convert::identity);
                Bar::default()
                    .label(series.name.clone())
                    .value(scaled)
                    .text_value(format!("{value:.1}"))
                    .style(theme::series(series_index))
                    .value_style(Style::default().fg(theme::TEXT))
            })
            .collect::<Vec<_>>();
        widget = widget.data(ratatui::widgets::BarGroup::new(bars).label(category.label.clone()));
    }
    frame.render_widget(widget, area);
}

fn draw_distribution_chart(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    chart: &chart::ChartModel,
    focused: bool,
) {
    let mut lines = Vec::new();
    for (series_index, series) in chart.series.iter().enumerate() {
        if chart.series.len() > 1 {
            lines.push(Line::styled(series.name.clone(), theme::header()));
        }
        let total = series
            .values
            .iter()
            .map(|value| value.value.abs())
            .sum::<f64>();
        for (value_index, value) in series.values.iter().enumerate() {
            let percent = if total == 0.0 {
                0.0
            } else {
                value.value.abs() * 100.0 / total
            };
            let cells = (1_u32..=40)
                .filter(|cell| f64::from(*cell) / 40.0 <= percent / 100.0)
                .count();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>16} ", value.label.chars().take(16).collect::<String>()),
                    Style::default().fg(theme::TEXT),
                ),
                Span::styled(
                    "█".repeat(cells),
                    Style::default().fg(theme::series(series_index + value_index)),
                ),
                Span::styled(format!(" {percent:5.1}%"), Style::default().fg(theme::TEXT)),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(focus_block(&chart.title, focused))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn index_as_f64(index: usize) -> f64 {
    u32::try_from(index).map_or(f64::from(u32::MAX), f64::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{ChartSeries, ChartValue};
    use loremesh_core::{ArtifactId, SnapshotId};
    use proptest::prelude::*;
    use ratatui::backend::TestBackend;

    struct Handler;
    impl CommandHandler for Handler {
        fn execute(&mut self, command: &SlashCommand, _: &ViewContent) -> CommandResponse {
            CommandResponse {
                message: format!("handled {command:?}"),
                content: None,
                input_mode: None,
            }
        }
    }

    fn view() -> DashboardView {
        DashboardView::from_domain("demo", &[], &[], None)
    }

    fn multi_series_chart(kind: chart::ChartKind) -> chart::ChartModel {
        chart::ChartModel::with_series(
            "Resource usage",
            kind,
            vec![
                ChartSeries {
                    name: "Current".into(),
                    values: vec![
                        ChartValue {
                            label: "node-1".into(),
                            value: 30.0,
                        },
                        ChartValue {
                            label: "node-2".into(),
                            value: 64.0,
                        },
                    ],
                },
                ChartSeries {
                    name: "Baseline".into(),
                    values: vec![
                        ChartValue {
                            label: "node-1".into(),
                            value: 24.0,
                        },
                        ChartValue {
                            label: "node-2".into(),
                            value: 51.0,
                        },
                    ],
                },
            ],
        )
        .expect("valid multi-series chart")
    }

    #[test]
    fn quit_and_exit_parse_as_clean_quit() {
        assert_eq!(parse_command("/quit"), Ok(SlashCommand::Quit));
        assert_eq!(parse_command("/exit"), Ok(SlashCommand::Quit));
    }

    #[test]
    fn investigation_commands_parse_into_typed_state_transitions() {
        assert_eq!(
            parse_command("/investigation new \"Feature Alpha Analysis\"")
                .expect("new investigation"),
            SlashCommand::Investigation(InvestigationCommand::New {
                title: "Feature Alpha Analysis".into(),
                organization: false,
            })
        );
        assert_eq!(
            parse_command("/investigation add current").expect("add current"),
            SlashCommand::Investigation(InvestigationCommand::AddCurrent)
        );
        assert_eq!(
            parse_command("/investigation status in-review").expect("status"),
            SlashCommand::Investigation(InvestigationCommand::Status("in-review".into()))
        );
        assert_eq!(
            parse_command(
                "/investigation export --format html --output reports/feature-alpha.html"
            )
            .expect("export"),
            SlashCommand::Investigation(InvestigationCommand::ExportHtml {
                output: "reports/feature-alpha.html".into(),
            })
        );
        assert!(parse_command("/investigation export --format pdf --output report.pdf").is_err());
    }

    #[test]
    fn save_command_is_typed() {
        assert_eq!(
            parse_command("/save current --format markdown-mermaid --output trace.md"),
            Ok(SlashCommand::Save {
                format: SaveFormat::MarkdownMermaid,
                output: Some("trace.md".into())
            })
        );
    }

    #[test]
    fn data_browser_chart_and_shell_commands_are_typed() {
        assert_eq!(
            parse_command("/table sort score desc"),
            Ok(SlashCommand::Table(TableCommand::Sort {
                column: "score".into(),
                descending: true,
            }))
        );
        assert!(matches!(
            parse_command("/chart hbar name score"),
            Ok(SlashCommand::Chart {
                kind: chart::ChartKind::HorizontalBar,
                ..
            })
        ));
        assert_eq!(
            parse_command("/open src/lib.rs"),
            Ok(SlashCommand::Browser(BrowserCommand::Open(
                "src/lib.rs".into()
            )))
        );
        assert_eq!(
            parse_command("/shell run echo hello"),
            Ok(SlashCommand::Shell(ShellCommand::Run("echo hello".into())))
        );
        assert_eq!(
            parse_command("/shell"),
            Ok(SlashCommand::Shell(ShellCommand::Enter))
        );
        assert_eq!(
            parse_command("/demo markdown"),
            Ok(SlashCommand::Demo(DemoKind::Markdown))
        );
        assert_eq!(
            parse_command("/search retry policy"),
            Ok(SlashCommand::KnowledgeSearch("retry policy".into()))
        );
        assert_eq!(
            parse_command("/find retry"),
            Ok(SlashCommand::Browser(BrowserCommand::Search(
                "retry".into()
            )))
        );
    }

    #[test]
    fn help_replaces_timeline_with_multiline_reference() {
        let mut state = ShellState {
            focus: Focus::Input,
            input: "/help".into(),
            ..ShellState::default()
        };
        state.submit(&view(), &mut Handler);
        let content = state.custom.expect("help content");
        assert_eq!(state.focus, Focus::Primary);
        assert!(content.paragraphs[0].contains("TABLES\n/table load"));
        assert!(content.paragraphs[0].contains("/demo shell"));
        assert!(content.paragraphs[0].contains("Start a persistent shell"));
    }

    #[test]
    fn escape_never_quits_and_timeline_scroll_is_bounded() {
        let mut state = ShellState::default();
        let view = view();
        let mut handler = Handler;
        state.show_content(ViewContent {
            title: "Long result".into(),
            paragraphs: vec![(0..25)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n")],
            table: None,
            chart: None,
            mermaid: None,
            d2: None,
        });
        state.handle_key(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.primary_scroll, 10);
        state.handle_key(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.primary_scroll, 25);
        for _ in 0..3 {
            state.handle_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &view,
                &mut handler,
            );
        }
        assert!(!state.should_quit());
        state.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.primary_scroll, 0);
    }

    #[test]
    fn shell_command_text_is_not_retained_in_history() {
        let mut state = ShellState {
            focus: Focus::Input,
            input: "/shell run echo private".into(),
            ..ShellState::default()
        };
        state.submit(&view(), &mut Handler);
        assert!(state.history.is_empty());
    }

    #[test]
    fn interactive_shell_routes_bare_input_and_exit_to_the_handler() {
        #[derive(Default)]
        struct ShellHandler {
            commands: Vec<SlashCommand>,
        }
        impl CommandHandler for ShellHandler {
            fn execute(&mut self, command: &SlashCommand, _: &ViewContent) -> CommandResponse {
                self.commands.push(command.clone());
                let input_mode = match command {
                    SlashCommand::Shell(ShellCommand::Enter) => Some(InputMode::Shell),
                    SlashCommand::Shell(ShellCommand::Exit) => Some(InputMode::LoreMesh),
                    _ => None,
                };
                CommandResponse {
                    message: String::new(),
                    content: None,
                    input_mode,
                }
            }
        }

        let view = view();
        let mut state = ShellState {
            focus: Focus::Input,
            input: "/shell".into(),
            ..ShellState::default()
        };
        let mut handler = ShellHandler::default();
        state.submit(&view, &mut handler);
        assert_eq!(state.input_mode, InputMode::Shell);
        assert_eq!(state.focus, Focus::Input);

        state.input = "echo hello".into();
        state.submit(&view, &mut handler);
        state.input = "/exit".into();
        state.submit(&view, &mut handler);

        assert_eq!(
            handler.commands,
            vec![
                SlashCommand::Shell(ShellCommand::Enter),
                SlashCommand::Shell(ShellCommand::Input("echo hello".into())),
                SlashCommand::Shell(ShellCommand::Exit),
            ]
        );
        assert_eq!(state.input_mode, InputMode::LoreMesh);
    }

    #[test]
    fn shell_focus_input_history_and_quit_are_deterministic() {
        let mut state = ShellState::default();
        let mut handler = Handler;
        let view = view();
        state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Timeline);
        state.handle_key(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        for character in "quit".chars() {
            state.handle_key(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                &view,
                &mut handler,
            );
        }
        state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert!(state.should_quit());
    }

    #[test]
    fn table_rows_select_and_focus_cycles_only_visible_regions() {
        let mut state = ShellState::default();
        let view = view();
        let mut handler = Handler;
        state.show_content(ViewContent {
            title: "Results".into(),
            paragraphs: Vec::new(),
            table: Some(ViewTable {
                columns: vec!["name".into()],
                rows: vec![vec!["one".into()], vec!["two".into()]],
            }),
            chart: None,
            mermaid: None,
            d2: None,
        });
        state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.selected_row, 1);
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, &view, &state))
            .expect("render selection");
        assert!(terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.bg == theme::FOCUS));
        state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Timeline);
        state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Input);
        state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Primary);
    }

    #[test]
    fn document_find_is_direct_and_back_restores_search_results() {
        let mut state = ShellState::default();
        let view = view();
        let mut handler = Handler;
        state.show_content(ViewContent {
            title: "Knowledge search".into(),
            paragraphs: vec!["Path: demo.md\n\nArchitecture body".into()],
            table: Some(ViewTable {
                columns: vec!["Title".into()],
                rows: vec![vec!["Architecture".into()]],
            }),
            chart: None,
            mermaid: None,
            d2: None,
        });
        state.show_content(ViewContent {
            title: "Architecture".into(),
            paragraphs: vec!["Path: architecture.md\n\ndocument body".into()],
            table: None,
            chart: None,
            mermaid: None,
            d2: None,
        });
        state.handle_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.input_mode, InputMode::Find);
        assert_eq!(state.focus, Focus::Input);
        state.input = "body".into();
        state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.input_mode, InputMode::LoreMesh);
        assert_eq!(state.content(&view).title, "Architecture");
        assert!(state
            .messages
            .back()
            .is_some_and(|message| message.contains("Browser(Search(\"body\"))")));
        assert_eq!(state.timeline_scroll, state.maximum_timeline_scroll());
        state.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.content(&view).title, "Knowledge search");
    }

    #[test]
    fn find_is_not_enabled_for_plain_text_panels() {
        let mut state = ShellState::default();
        let view = view();
        let mut handler = Handler;
        state.show_content(text_content("Help", "plain text panel"));
        state.handle_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Primary);
        assert_eq!(state.input_mode, InputMode::LoreMesh);
    }

    #[test]
    fn layered_layout_renders_with_test_backend() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let state = ShellState::default();
        let view = view();
        terminal
            .draw(|frame| draw(frame, &view, &state))
            .expect("render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(rendered.contains("Investigation timeline"));
        assert!(rendered.contains("Lineage"));
        assert!(rendered.contains("Command"));
    }

    #[test]
    fn structured_results_use_full_width_and_semantic_table_colors() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut state = ShellState::default();
        state.show_content(ViewContent {
            title: "Empty findings".into(),
            paragraphs: Vec::new(),
            table: Some(ViewTable {
                columns: vec!["Status".into(), "Count".into()],
                rows: Vec::new(),
            }),
            chart: None,
            mermaid: None,
            d2: None,
        });
        terminal
            .draw(|frame| draw(frame, &view(), &state))
            .expect("render table");
        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(rendered.contains("no results"));
        assert!(rendered.contains("Investigation timeline"));
        assert!(buffer.content.iter().any(|cell| cell.fg == theme::PRIMARY));
    }

    #[test]
    fn every_structured_chart_renderer_is_colored_and_labelled() {
        for kind in [
            chart::ChartKind::Bar,
            chart::ChartKind::HorizontalBar,
            chart::ChartKind::Line,
            chart::ChartKind::Pie,
        ] {
            let backend = TestBackend::new(100, 30);
            let mut terminal = Terminal::new(backend).expect("terminal");
            let mut state = ShellState::default();
            state.show_content(ViewContent {
                title: "Resource usage".into(),
                paragraphs: Vec::new(),
                table: None,
                chart: Some(multi_series_chart(kind)),
                mermaid: None,
                d2: None,
            });
            terminal
                .draw(|frame| draw(frame, &view(), &state))
                .expect("render chart");
            let buffer = terminal.backend().buffer();
            let rendered = buffer
                .content
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            assert!(rendered.contains("Resource usage"));
            assert!(rendered.contains("Current") || rendered.contains("Baseline"));
            assert!(buffer
                .content
                .iter()
                .any(|cell| cell.fg == theme::PRIMARY || cell.fg == theme::SECONDARY));
        }
    }

    #[test]
    fn narrow_chart_uses_readable_text_fallback() {
        let backend = TestBackend::new(50, 16);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut state = ShellState::default();
        state.show_content(ViewContent {
            title: "Resource usage".into(),
            paragraphs: Vec::new(),
            table: None,
            chart: Some(multi_series_chart(chart::ChartKind::Line)),
            mermaid: None,
            d2: None,
        });
        terminal
            .draw(|frame| draw(frame, &view(), &state))
            .expect("render compact chart");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(rendered.contains("compact view"));
        assert!(rendered.contains("Current"));
    }

    #[test]
    fn artifacts_project_as_structured_table() {
        let artifact = Artifact::new(
            ArtifactId::deterministic("a"),
            SnapshotId::deterministic("s"),
            "sample.md",
            10,
        )
        .expect("artifact");
        let projected = DashboardView::from_domain("demo", &[artifact], &[], None);
        assert_eq!(
            projected.artifacts.table.expect("table").rows[0][0],
            "sample.md"
        );
    }

    proptest! {
        #[test]
        fn arbitrary_command_text_never_panics(input in "[^\\p{C}]{0,200}") {
            let _result = parse_command(&input);
        }
    }
}
