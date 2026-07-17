use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use loremesh_tui::browser::{neutralize_terminal, CodeDocument, FileTreeEntry};
use loremesh_tui::chart::{ChartKind, ChartModel};
use loremesh_tui::grid::{DataGrid, SortDirection};
use loremesh_tui::markdown::MarkdownDocument;
use loremesh_tui::{BrowserCommand, ShellCommand, TableCommand, ViewContent, ViewTable};

use crate::{safe_workspace_output, TuiCommandHandler};

const MAX_VIEW_BYTES: usize = 1024 * 1024;
const MAX_PROCESS_OUTPUT: usize = 64 * 1024;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(10);

impl TuiCommandHandler {
    pub(super) fn table_command(
        &mut self,
        command: &TableCommand,
    ) -> Result<(String, Option<ViewContent>)> {
        match command {
            TableCommand::Load(path) => {
                let relative = PathBuf::from(path);
                let resolved = resolve_existing(&self.root, &relative)?;
                let bytes = read_bounded_file(&resolved, MAX_VIEW_BYTES)?;
                self.grid = Some(DataGrid::from_csv(&bytes)?);
                self.grid_source = Some(relative);
            }
            TableCommand::Refresh => {
                let relative = self
                    .grid_source
                    .as_ref()
                    .context("no CSV has been loaded")?;
                let resolved = resolve_existing(&self.root, relative)?;
                self.grid = Some(DataGrid::from_csv(&read_bounded_file(
                    &resolved,
                    MAX_VIEW_BYTES,
                )?)?);
            }
            TableCommand::Save(path) => {
                let grid = self.grid.as_ref().context("no CSV has been loaded")?;
                let relative = PathBuf::from(path);
                let output = safe_workspace_output(&self.root, &relative)?;
                if output.exists() {
                    bail!("refusing to overwrite existing output {}", output.display());
                }
                if let Some(parent) = output.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&output, grid.projection_csv()?)?;
                return Ok((format!("Saved {}", relative.display()), None));
            }
            TableCommand::Sort { column, descending } => self.grid_mut()?.sort(
                column,
                if *descending {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                },
            )?,
            TableCommand::Filter { column, value } => self.grid_mut()?.filter(column, value)?,
            TableCommand::Search(query) => self.grid_mut()?.search(query),
            TableCommand::Columns(columns) => self.grid_mut()?.show_columns(columns)?,
            TableCommand::Reset => self.grid_mut()?.reset(),
        }
        let view = self.grid_view()?;
        Ok((
            format!(
                "Table: {} matching row(s)",
                self.grid_mut()?.matching_rows()
            ),
            Some(view),
        ))
    }

    pub(super) fn chart_command(
        &mut self,
        kind: ChartKind,
        label_column: &str,
        value_column: &str,
    ) -> Result<(String, Option<ViewContent>)> {
        let pairs = self.grid_mut()?.value_pairs(label_column, value_column)?;
        let chart =
            ChartModel::from_pairs(format!("{value_column} by {label_column}"), kind, pairs)?;
        let view = ViewContent {
            title: chart.title.clone(),
            paragraphs: vec![chart.render_text(80)],
            table: None,
            mermaid: None,
            d2: None,
        };
        Ok((format!("Rendered {kind:?} chart"), Some(view)))
    }

    pub(super) fn browser_command(
        &mut self,
        command: &BrowserCommand,
    ) -> Result<(String, Option<ViewContent>)> {
        match command {
            BrowserCommand::Browse(path) => {
                let relative = PathBuf::from(path.as_deref().unwrap_or("."));
                let entries = list_directory(&self.root, &relative)?;
                let rows = entries
                    .iter()
                    .map(|entry| {
                        vec![
                            format!("{}{}", "  ".repeat(entry.depth), entry.relative_path),
                            if entry.is_directory {
                                "directory"
                            } else {
                                "file"
                            }
                            .into(),
                        ]
                    })
                    .collect();
                Ok((
                    format!("Listed {} entries", entries.len()),
                    Some(ViewContent {
                        title: "Code browser".into(),
                        paragraphs: vec!["Read-only workspace view".into()],
                        table: Some(ViewTable {
                            columns: vec!["Path".into(), "Kind".into()],
                            rows,
                        }),
                        mermaid: None,
                        d2: None,
                    }),
                ))
            }
            BrowserCommand::Open(path) => {
                let relative = PathBuf::from(path);
                let resolved = resolve_existing(&self.root, &relative)?;
                let bytes = read_bounded_file(&resolved, MAX_VIEW_BYTES)?;
                let document = CodeDocument::from_bytes(path, &bytes, MAX_VIEW_BYTES)?;
                let rendered = if resolved.extension().is_some_and(|value| value == "md") {
                    MarkdownDocument::parse(std::str::from_utf8(&bytes)?).render_text()
                } else {
                    document.numbered_text()
                };
                let lines = document.line_count();
                self.code_document = Some(document);
                Ok((
                    format!("Opened {path} ({lines} lines)"),
                    Some(text_view(path, rendered)),
                ))
            }
            BrowserCommand::Search(query) => {
                let document = self
                    .code_document
                    .as_ref()
                    .context("no code document is open")?;
                let matches = document.search(query, false);
                let summary = matches
                    .iter()
                    .map(|value| format!("line {}, column {}", value.line, value.column))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok((
                    format!("Found {} match(es)", matches.len()),
                    Some(text_view("Search results", summary)),
                ))
            }
        }
    }

    pub(super) fn shell_command(
        &mut self,
        command: &ShellCommand,
    ) -> Result<(String, Option<ViewContent>)> {
        match command {
            ShellCommand::Status => Ok((
                format!(
                    "Local shell is {}",
                    if self.shell_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                None,
            )),
            ShellCommand::Enable => {
                self.shell_enabled = true;
                Ok(("Local shell enabled for this session. Commands have your OS permissions and may access files or networks.".into(), None))
            }
            ShellCommand::Disable => {
                self.shell_enabled = false;
                Ok(("Local shell disabled".into(), None))
            }
            ShellCommand::Run(command) => {
                if !self.shell_enabled {
                    bail!("local shell is disabled; run /shell enable first");
                }
                let result = run_local_shell(command, &self.root)?;
                Ok((
                    format!(
                        "Shell exited {}{}",
                        result.status,
                        if result.truncated {
                            " (output truncated)"
                        } else {
                            ""
                        }
                    ),
                    Some(text_view("Untrusted local shell output", result.output)),
                ))
            }
        }
    }

    fn grid_mut(&mut self) -> Result<&mut DataGrid> {
        self.grid.as_mut().context("no CSV has been loaded")
    }

    fn grid_view(&self) -> Result<ViewContent> {
        let grid = self.grid.as_ref().context("no CSV has been loaded")?;
        Ok(ViewContent {
            title: "Data table".into(),
            paragraphs: vec![format!(
                "{} matching of {} total rows",
                grid.matching_rows(),
                grid.total_rows()
            )],
            table: Some(grid.projection()),
            mermaid: None,
            d2: None,
        })
    }
}

fn text_view(title: &str, text: String) -> ViewContent {
    ViewContent {
        title: title.into(),
        paragraphs: vec![text],
        table: None,
        mermaid: None,
        d2: None,
    }
}

fn resolve_existing(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || relative.starts_with(".loremesh")
        || relative.starts_with(".git")
    {
        bail!("path must remain inside the visible workspace");
    }
    let canonical_root = root.canonicalize()?;
    let resolved = root.join(relative).canonicalize()?;
    if !resolved.starts_with(&canonical_root) {
        bail!("path resolves outside the workspace");
    }
    Ok(resolved)
}

fn read_bounded_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        bail!("{} is not a file", path.display());
    }
    if metadata.len() > limit as u64 {
        bail!("file exceeds the {limit} byte viewing limit");
    }
    fs::read(path).with_context(|| format!("could not read {}", path.display()))
}

fn list_directory(root: &Path, relative: &Path) -> Result<Vec<FileTreeEntry>> {
    let directory = resolve_existing(root, relative)?;
    if !directory.is_dir() {
        bail!("{} is not a directory", relative.display());
    }
    let mut entries = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if matches!(name.as_str(), ".git" | ".loremesh" | "target") {
                return None;
            }
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            Some(FileTreeEntry {
                relative_path: entry
                    .path()
                    .strip_prefix(root)
                    .ok()?
                    .to_string_lossy()
                    .into_owned(),
                is_directory: file_type.is_dir(),
                depth: 0,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(entries)
}

struct ProcessResult {
    status: String,
    output: String,
    truncated: bool,
}

fn run_local_shell(command: &str, root: &Path) -> Result<ProcessResult> {
    #[cfg(windows)]
    let mut child = Command::new("cmd")
        .args(["/C", command])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    #[cfg(not(windows))]
    let mut child = Command::new("sh")
        .args(["-c", command])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().context("could not capture stdout")?;
    let stderr = child.stderr.take().context("could not capture stderr")?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, MAX_PROCESS_OUTPUT));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_PROCESS_OUTPUT));
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            let _status = child.wait()?;
            bail!("shell command exceeded the 10 second deadline");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader failed"))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader failed"))??;
    let output = format!(
        "stdout:\n{}\n\nstderr:\n{}",
        neutralize_terminal(&String::from_utf8_lossy(&stdout)),
        neutralize_terminal(&String::from_utf8_lossy(&stderr))
    );
    Ok(ProcessResult {
        status: status
            .code()
            .map_or_else(|| "by signal".into(), |code| format!("with code {code}")),
        output,
        truncated: stdout_truncated || stderr_truncated,
    })
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(retained.len());
        retained.write_all(&buffer[..read.min(remaining)])?;
        truncated |= read > remaining;
    }
    Ok((retained, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(root: &Path) -> TuiCommandHandler {
        TuiCommandHandler::new(root.to_path_buf())
    }

    #[test]
    fn traversal_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        assert!(resolve_existing(temporary.path(), Path::new("../outside")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        symlink(outside.path(), workspace.path().join("escape")).expect("symlink fixture");
        assert!(resolve_existing(workspace.path(), Path::new("escape")).is_err());
    }

    #[test]
    fn table_load_transform_and_save_stay_in_workspace() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        fs::write(
            temporary.path().join("input.csv"),
            "name,score\nBeta,2\nAlpha,10\n",
        )
        .expect("CSV fixture");
        let mut workbench = handler(temporary.path());
        let (_, view) = workbench
            .table_command(&TableCommand::Load("input.csv".into()))
            .expect("load table");
        assert_eq!(
            view.expect("table view").table.expect("table").rows.len(),
            2
        );
        workbench
            .table_command(&TableCommand::Sort {
                column: "score".into(),
                descending: true,
            })
            .expect("sort table");
        workbench
            .table_command(&TableCommand::Save("output.csv".into()))
            .expect("save table");
        let saved = fs::read_to_string(temporary.path().join("output.csv")).expect("saved CSV");
        assert!(saved.find("Alpha,10").expect("Alpha") < saved.find("Beta,2").expect("Beta"));
    }

    #[test]
    fn markdown_open_and_search_are_offline_and_content_safe() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        fs::write(
            temporary.path().join("notes.md"),
            "# Notes\n\n```d2\nA -> B\n```\n",
        )
        .expect("Markdown fixture");
        let mut workbench = handler(temporary.path());
        let (_, view) = workbench
            .browser_command(&BrowserCommand::Open("notes.md".into()))
            .expect("open Markdown");
        assert!(view.expect("Markdown view").paragraphs[0].contains("A ──▶ B"));
        let (message, _) = workbench
            .browser_command(&BrowserCommand::Search("notes".into()))
            .expect("search open document");
        assert_eq!(message, "Found 1 match(es)");
    }

    #[test]
    fn shell_is_disabled_until_explicitly_enabled() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let mut workbench = handler(temporary.path());
        assert!(workbench
            .shell_command(&ShellCommand::Run("echo blocked".into()))
            .is_err());
        workbench
            .shell_command(&ShellCommand::Enable)
            .expect("enable shell");
        assert!(workbench
            .shell_command(&ShellCommand::Run("echo allowed".into()))
            .is_ok());
    }

    #[test]
    fn shell_output_is_captured_and_labelled() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let result =
            run_local_shell("echo hello", temporary.path()).expect("local deterministic command");
        assert!(result.output.starts_with("stdout:"));
        assert!(result.output.contains("hello"));
    }
}
