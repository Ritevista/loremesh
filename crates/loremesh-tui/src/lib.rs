//! Reusable interactive terminal shell for `LoreMesh` workbench views.
#![forbid(unsafe_code)]

pub mod browser;
pub mod chart;
pub mod grid;
pub mod markdown;

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
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewContent {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub table: Option<ViewTable>,
    pub mermaid: Option<String>,
    pub d2: Option<String>,
}

impl ViewContent {
    fn detail_text(&self) -> String {
        let mut text = self.paragraphs.join("\n\n");
        if let Some(table) = &self.table {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            let _ = write!(
                text,
                "{} rows · {} columns",
                table.rows.len(),
                table.columns.len()
            );
        }
        text
    }
}

/// Pure presentation data projected from `LoreMesh` domain state.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Timeline,
    Context,
    Input,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Timeline => Self::Context,
            Self::Context => Self::Input,
            Self::Input => Self::Timeline,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Timeline => Self::Input,
            Self::Context => Self::Timeline,
            Self::Input => Self::Context,
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
    Status,
    Enable,
    Disable,
    Run(String),
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
        "/search" => one_rest(parts, "usage: /search <text>")
            .map(|query| SlashCommand::Browser(BrowserCommand::Search(query))),
        "/quit" | "/exit" => no_args(parts, SlashCommand::Quit),
        "/save" | "/export" => parse_save(parts),
        _ => Err(CommandError(format!("unknown command '{name}'; use /help"))),
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResponse {
    pub message: String,
    pub content: Option<ViewContent>,
}

/// Application boundary for commands that require services or filesystem I/O.
pub trait CommandHandler {
    fn execute(&mut self, command: &SlashCommand, active: &ViewContent) -> CommandResponse;
}

/// Pure interactive shell state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellState {
    pub focus: Focus,
    pub input: String,
    pub active: ViewKind,
    pub messages: VecDeque<String>,
    history: VecDeque<String>,
    history_cursor: Option<usize>,
    should_quit: bool,
    custom: Option<ViewContent>,
    timeline_scroll: u16,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            focus: Focus::Timeline,
            input: String::new(),
            active: ViewKind::Summary,
            messages: VecDeque::from([String::from(
                "Ready. Press / for commands or Tab to change focus.",
            )]),
            history: VecDeque::new(),
            history_cursor: None,
            should_quit: false,
            custom: None,
            timeline_scroll: 0,
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
            KeyCode::Esc if self.focus == Focus::Input => {
                self.input.clear();
                self.focus = Focus::Timeline;
                self.history_cursor = None;
            }
            KeyCode::Esc => self.focus = Focus::Timeline,
            KeyCode::PageUp => {
                self.focus = Focus::Timeline;
                self.timeline_scroll = self.timeline_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.focus = Focus::Timeline;
                self.timeline_scroll = self
                    .timeline_scroll
                    .saturating_add(10)
                    .min(self.maximum_scroll(view));
            }
            KeyCode::Home if self.focus != Focus::Input => self.timeline_scroll = 0,
            KeyCode::End if self.focus != Focus::Input => {
                self.timeline_scroll = self.maximum_scroll(view);
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
                self.focus = Focus::Timeline;
                self.timeline_scroll = 0;
                self.push_message(format!("Opened {}", view.content(kind).title));
            }
            Ok(SlashCommand::Clear) => {
                self.messages.clear();
                self.focus = Focus::Timeline;
            }
            Ok(SlashCommand::Quit) => self.should_quit = true,
            Ok(command) => {
                let response = handler.execute(&command, self.content(view));
                self.push_message(response.message);
                self.focus = Focus::Timeline;
                if let Some(content) = response.content {
                    self.show_content(content);
                }
            }
            Err(error) => {
                let message = error.to_string();
                self.show_content(text_content("Command error", &message));
                self.push_message(message);
            }
        }
    }

    fn show_content(&mut self, content: ViewContent) {
        self.custom = Some(content);
        self.active = ViewKind::Custom;
        self.focus = Focus::Timeline;
        self.timeline_scroll = 0;
    }

    fn maximum_scroll(&self, view: &DashboardView) -> u16 {
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
        mermaid: None,
        d2: None,
    }
}

fn help_content() -> ViewContent {
    text_content(
        "LoreMesh command reference",
        "LOREMESH COMMAND REFERENCE\n\nNAVIGATION\n/help\n  Show this complete reference.\n/artifacts\n  Show imported artifacts.\n/findings\n  Show findings.\n/trace\n  Show evidence lineage.\n\nDEMONSTRATIONS\n/demo table\n/demo chart\n/demo markdown\n/demo code\n/demo shell\n  Open deterministic capability previews; no files, network, model, or shell execution required.\n\nTABLES\n/table load <workspace-relative.csv>\n/table refresh\n/table save <workspace-relative.csv>\n/table sort <column> <asc|desc>\n/table filter <column> <text>\n/table search <text>\n/table columns <column,...>\n/table reset\n\nCHARTS\n/chart <bar|hbar|line|pie> <label-column> <value-column>\n  Requires a loaded table. Example: /chart hbar name duration\n\nFILES AND MARKDOWN\n/browse [workspace-relative-directory]\n/open <workspace-relative-file>\n/search <text>\n\nLOCAL SHELL\n/shell status\n/shell enable\n/shell run <command>\n/shell disable\n  Shell execution is non-interactive, disabled at startup, limited to 10 seconds, and has your OS permissions. Command text is not retained in history.\n\nREPORTS\n/save current --format <md|markdown-mermaid|markdown-d2|csv|html|png> [--output <path>]\n/export current --format <format> [--output <path>]\n\nSERVICES\n/services\n/model\n/context\n/compact\n/clear\n\nEXIT\n/quit\n/exit\n  Outside input, q exits. Esc always returns focus to the timeline and never exits.",
    )
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
        terminal.draw(|frame| draw(frame, view, state))?;
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
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
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
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "LoreMesh",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" · {} · offline", view.workspace_name)),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        rows[0],
    );
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(rows[1]);
    draw_timeline(frame, body[0], view, state);
    let active = state.content(view);
    frame.render_widget(
        Paragraph::new(active.detail_text())
            .wrap(Wrap { trim: false })
            .block(focus_block(&active.title, state.focus == Focus::Context)),
        body[1],
    );
    let input_style = if state.focus == Focus::Input {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(format!("> {}", state.input))
            .style(input_style)
            .block(focus_block("Command", state.focus == Focus::Input)),
        rows[2],
    );
    let latest = state.messages.back().map_or("ready", String::as_str);
    frame.render_widget(
        Paragraph::new(format!(
            "Tab focus · PgUp/PgDn Home/End scroll · / command · q or /quit exit · {latest}"
        ))
        .style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
}

fn draw_timeline(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &DashboardView,
    state: &ShellState,
) {
    let active = state.content(view);
    if let Some(table) = &active.table {
        let widths = (0..table.columns.len())
            .map(|_| Constraint::Fill(1))
            .collect::<Vec<_>>();
        let rows = table
            .rows
            .iter()
            .skip(usize::from(state.timeline_scroll))
            .map(|row| Row::new(row.iter().cloned().map(Cell::from)));
        frame.render_widget(
            Table::new(rows, widths)
                .header(
                    Row::new(table.columns.clone()).style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                )
                .block(focus_block(
                    "Timeline / results",
                    state.focus == Focus::Timeline,
                )),
            area,
        );
    } else {
        let mut lines = active.paragraphs.clone();
        if !state.messages.is_empty() {
            lines.push(String::new());
            lines.extend(state.messages.iter().map(|message| format!("› {message}")));
        }
        frame.render_widget(
            Paragraph::new(lines.join("\n"))
                .scroll((state.timeline_scroll, 0))
                .wrap(Wrap { trim: false })
                .block(focus_block(
                    "Investigation timeline",
                    state.focus == Focus::Timeline,
                )),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loremesh_core::{ArtifactId, SnapshotId};
    use proptest::prelude::*;
    use ratatui::backend::TestBackend;

    struct Handler;
    impl CommandHandler for Handler {
        fn execute(&mut self, command: &SlashCommand, _: &ViewContent) -> CommandResponse {
            CommandResponse {
                message: format!("handled {command:?}"),
                content: None,
            }
        }
    }

    fn view() -> DashboardView {
        DashboardView::from_domain("demo", &[], &[], None)
    }

    #[test]
    fn quit_and_exit_parse_as_clean_quit() {
        assert_eq!(parse_command("/quit"), Ok(SlashCommand::Quit));
        assert_eq!(parse_command("/exit"), Ok(SlashCommand::Quit));
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
            parse_command("/demo markdown"),
            Ok(SlashCommand::Demo(DemoKind::Markdown))
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
        assert_eq!(state.focus, Focus::Timeline);
        assert!(content.paragraphs[0].contains("TABLES\n/table load"));
        assert!(content.paragraphs[0].contains("/demo shell"));
        assert!(content.paragraphs[0].contains("/shell run <command>"));
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
            mermaid: None,
            d2: None,
        });
        state.handle_key(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.timeline_scroll, 10);
        state.handle_key(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.timeline_scroll, 25);
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
        assert_eq!(state.timeline_scroll, 0);
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
    fn shell_focus_input_history_and_quit_are_deterministic() {
        let mut state = ShellState::default();
        let mut handler = Handler;
        let view = view();
        state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &view,
            &mut handler,
        );
        assert_eq!(state.focus, Focus::Context);
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
        assert!(rendered.contains("Command"));
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
