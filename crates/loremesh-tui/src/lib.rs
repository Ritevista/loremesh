//! Minimal `LoreMesh` terminal dashboard and testable view model.
#![forbid(unsafe_code)]

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use loremesh_core::{Artifact, Finding, Trace};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Terminal;
use thiserror::Error;

/// Terminal lifecycle failure.
#[derive(Debug, Error)]
#[error("terminal operation failed: {0}")]
pub struct TuiError(#[from] io::Error);

/// Pure presentation state consumed by the terminal renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardView {
    pub workspace_name: String,
    pub artifacts: Vec<(String, String)>,
    pub findings: Vec<(String, String)>,
    pub selected_finding: String,
    pub lineage: Vec<String>,
}

impl DashboardView {
    /// Projects domain state without terminal I/O.
    pub fn from_domain(
        workspace_name: &str,
        artifacts: &[Artifact],
        findings: &[Finding],
        trace: Option<&Trace>,
    ) -> Self {
        let selected_finding = findings.first().map_or_else(
            || "No findings".into(),
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
        let lineage = trace
            .map(|value| {
                value
                    .nodes()
                    .map(|node| format!("{}: {}", node.id, node.label))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            workspace_name: workspace_name.into(),
            artifacts: artifacts
                .iter()
                .map(|artifact| (artifact.name.clone(), artifact.id.to_string()))
                .collect(),
            findings: findings
                .iter()
                .map(|finding| (finding.title.clone(), format!("{:?}", finding.status)))
                .collect(),
            selected_finding,
            lineage,
        }
    }
}

/// Runs the dashboard until `q` or Escape is pressed.
pub fn run(view: &DashboardView) -> Result<(), TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, view);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    view: &DashboardView,
) -> Result<(), TuiError> {
    loop {
        terminal.draw(|frame| draw(frame, view))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    return Ok(());
                }
            }
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, view: &DashboardView) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!("Workspace: {}  •  q to quit", view.workspace_name))
            .block(Block::default().title("LoreMesh").borders(Borders::ALL)),
        vertical[0],
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(vertical[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(columns[1]);
    let artifact_rows = view
        .artifacts
        .iter()
        .map(|(name, id)| Row::new(vec![Cell::from(name.clone()), Cell::from(id.clone())]));
    frame.render_widget(
        Table::new(
            artifact_rows,
            [Constraint::Percentage(35), Constraint::Percentage(65)],
        )
        .header(Row::new(["Artifact", "ID"]))
        .block(Block::default().title("Artifacts").borders(Borders::ALL)),
        left[0],
    );
    let finding_rows = view.findings.iter().map(|(title, status)| {
        Row::new(vec![Cell::from(title.clone()), Cell::from(status.clone())])
    });
    frame.render_widget(
        Table::new(
            finding_rows,
            [Constraint::Percentage(65), Constraint::Percentage(35)],
        )
        .header(Row::new(["Finding", "Status"]))
        .block(Block::default().title("Findings").borders(Borders::ALL)),
        left[1],
    );
    frame.render_widget(
        Paragraph::new(view.selected_finding.clone()).block(
            Block::default()
                .title("Selected finding")
                .borders(Borders::ALL),
        ),
        right[0],
    );
    frame.render_widget(
        Paragraph::new(view.lineage.join("\n↓\n")).block(
            Block::default()
                .title("Evidence path / lineage")
                .borders(Borders::ALL),
        ),
        right[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use loremesh_core::{ArtifactId, SnapshotId};

    #[test]
    fn empty_domain_has_explicit_selected_state() {
        let view = DashboardView::from_domain("demo", &[], &[], None);
        assert_eq!(view.selected_finding, "No findings");
        assert!(view.lineage.is_empty());
    }

    #[test]
    fn artifacts_project_without_terminal() {
        let artifact = Artifact::new(
            ArtifactId::deterministic("a"),
            SnapshotId::deterministic("s"),
            "sample.md",
            10,
        )
        .expect("artifact");
        let view = DashboardView::from_domain("demo", &[artifact], &[], None);
        assert_eq!(view.artifacts[0].0, "sample.md");
    }
}
