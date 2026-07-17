//! Renderer-independent `LoreMesh` report models and deterministic exporters.
#![forbid(unsafe_code)]

use std::fmt::Write as _;

use loremesh_core::ReportId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Report validation or rendering error.
#[derive(Debug, Error)]
pub enum ReportError {
    #[error("invalid report {field}: {reason}")]
    Validation { field: &'static str, reason: String },
    #[error("report has no table to export as CSV")]
    NoTable,
    #[error("serialization failed: {0}")]
    Serialization(String),
}

/// Renderer-independent report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub id: ReportId,
    pub title: String,
    pub sections: Vec<ReportSection>,
}

impl Report {
    pub fn new(
        id: ReportId,
        title: impl Into<String>,
        sections: Vec<ReportSection>,
    ) -> Result<Self, ReportError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(invalid("title", "must not be blank"));
        }
        Ok(Self {
            id,
            title,
            sections,
        })
    }
}

/// Ordered section in a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub blocks: Vec<ReportBlock>,
}

impl ReportSection {
    pub fn new(title: impl Into<String>, blocks: Vec<ReportBlock>) -> Result<Self, ReportError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(invalid("section title", "must not be blank"));
        }
        Ok(Self { title, blocks })
    }
}

/// Supported report content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ReportBlock {
    Paragraph(String),
    Table(TableModel),
    Metric(Metric),
}

/// Rectangular table model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableModel {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl TableModel {
    pub fn new(
        title: impl Into<String>,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> Result<Self, ReportError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(invalid("table title", "must not be blank"));
        }
        if columns.is_empty() || columns.iter().any(|column| column.trim().is_empty()) {
            return Err(invalid("table columns", "must be non-empty and non-blank"));
        }
        let mut unique = columns.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != columns.len() {
            return Err(invalid("table columns", "must be unique"));
        }
        if rows.iter().any(|row| row.len() != columns.len()) {
            return Err(invalid(
                "table rows",
                "every row must match the column count",
            ));
        }
        Ok(Self {
            title,
            columns,
            rows,
        })
    }
}

/// Display-ready scalar metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metric {
    pub label: String,
    pub value: String,
    pub unit: Option<String>,
}

impl Metric {
    pub fn new(
        label: impl Into<String>,
        value: impl Into<String>,
        unit: Option<String>,
    ) -> Result<Self, ReportError> {
        let label = label.into();
        let value = value.into();
        if label.trim().is_empty() {
            return Err(invalid("metric label", "must not be blank"));
        }
        if value.trim().is_empty() {
            return Err(invalid("metric value", "must not be blank"));
        }
        Ok(Self { label, value, unit })
    }
}

fn invalid(field: &'static str, reason: &str) -> ReportError {
    ReportError::Validation {
        field,
        reason: reason.into(),
    }
}

/// Serializes the complete report as stable pretty JSON.
pub fn render_json(report: &Report) -> Result<String, ReportError> {
    let mut value = serde_json::to_string_pretty(report)
        .map_err(|error| ReportError::Serialization(error.to_string()))?;
    value.push('\n');
    Ok(value)
}

/// Serializes the first report table as RFC-compatible CSV.
pub fn render_csv(report: &Report) -> Result<String, ReportError> {
    let table = report
        .sections
        .iter()
        .flat_map(|section| &section.blocks)
        .find_map(|block| match block {
            ReportBlock::Table(table) => Some(table),
            _ => None,
        })
        .ok_or(ReportError::NoTable)?;
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer
        .write_record(&table.columns)
        .map_err(|error| ReportError::Serialization(error.to_string()))?;
    for row in &table.rows {
        writer
            .write_record(row)
            .map_err(|error| ReportError::Serialization(error.to_string()))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|error| ReportError::Serialization(error.to_string()))?;
    String::from_utf8(bytes).map_err(|error| ReportError::Serialization(error.to_string()))
}

/// Renders portable Markdown.
pub fn render_markdown(report: &Report) -> String {
    let mut output = format!("# {}\n", report.title);
    for section in &report.sections {
        let _ = write!(output, "\n## {}\n", section.title);
        for block in &section.blocks {
            match block {
                ReportBlock::Paragraph(text) => {
                    let _ = write!(output, "\n{text}\n");
                }
                ReportBlock::Metric(metric) => {
                    let unit = metric.unit.as_deref().unwrap_or("");
                    let _ = write!(
                        output,
                        "\n- **{}:** {}{}\n",
                        metric.label, metric.value, unit
                    );
                }
                ReportBlock::Table(table) => {
                    let _ = write!(
                        output,
                        "\n### {}\n\n| {} |\n| {} |\n",
                        table.title,
                        table
                            .columns
                            .iter()
                            .map(|v| escape_markdown(v))
                            .collect::<Vec<_>>()
                            .join(" | "),
                        table
                            .columns
                            .iter()
                            .map(|_| "---")
                            .collect::<Vec<_>>()
                            .join(" | ")
                    );
                    for row in &table.rows {
                        let _ = writeln!(
                            output,
                            "| {} |",
                            row.iter()
                                .map(|v| escape_markdown(v))
                                .collect::<Vec<_>>()
                                .join(" | ")
                        );
                    }
                }
            }
        }
    }
    output
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

/// Renders self-contained HTML without scripts or remote assets.
pub fn render_html(report: &Report) -> String {
    let mut output = String::from("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>");
    output.push_str(&escape_html(&report.title));
    output.push_str("</title><style>body{font-family:system-ui,sans-serif;max-width:72rem;margin:2rem auto;padding:0 1rem;color:#17202a}table{border-collapse:collapse;width:100%}th,td{border:1px solid #ccd1d1;padding:.5rem;text-align:left}th{background:#f4f6f7}.metric{font-size:1.1rem}</style></head><body>");
    let _ = write!(output, "<h1>{}</h1>", escape_html(&report.title));
    for section in &report.sections {
        let _ = write!(output, "<section><h2>{}</h2>", escape_html(&section.title));
        for block in &section.blocks {
            match block {
                ReportBlock::Paragraph(text) => {
                    let _ = write!(output, "<p>{}</p>", escape_html(text));
                }
                ReportBlock::Metric(metric) => {
                    let _ = write!(
                        output,
                        "<p class=\"metric\"><strong>{}:</strong> {}{}</p>",
                        escape_html(&metric.label),
                        escape_html(&metric.value),
                        metric.unit.as_deref().map(escape_html).unwrap_or_default()
                    );
                }
                ReportBlock::Table(table) => {
                    let _ = write!(
                        output,
                        "<h3>{}</h3><table><thead><tr>",
                        escape_html(&table.title)
                    );
                    for column in &table.columns {
                        let _ = write!(output, "<th>{}</th>", escape_html(column));
                    }
                    output.push_str("</tr></thead><tbody>");
                    for row in &table.rows {
                        output.push_str("<tr>");
                        for value in row {
                            let _ = write!(output, "<td>{}</td>", escape_html(value));
                        }
                        output.push_str("</tr>");
                    }
                    output.push_str("</tbody></table>");
                }
            }
        }
        output.push_str("</section>");
    }
    output.push_str("</body></html>\n");
    output
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sample_report(value: &str) -> Report {
        let table = TableModel::new(
            "Artifacts",
            vec!["Name".into(), "Kind".into()],
            vec![vec![value.into(), "Markdown".into()]],
        )
        .expect("valid table");
        Report::new(
            ReportId::deterministic("sample"),
            "Demo",
            vec![
                ReportSection::new("Inventory", vec![ReportBlock::Table(table)])
                    .expect("valid section"),
            ],
        )
        .expect("valid report")
    }

    #[test]
    fn html_escapes_untrusted_values() {
        let html = render_html(&sample_report("<script>alert('x')</script>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn markdown_and_html_are_stable() {
        let report = sample_report("sample.md");
        assert_eq!(
            render_markdown(&report),
            include_str!("../../../fixtures/demo/report.md")
        );
        assert_eq!(
            render_html(&report),
            include_str!("../../../fixtures/demo/report.html")
        );
    }

    proptest! {
        #[test]
        fn rectangular_tables_serialize(rows in prop::collection::vec(prop::collection::vec("[a-z]{0,8}", 2), 0..20)) {
            let table = TableModel::new("T", vec!["A".into(), "B".into()], rows);
            prop_assert!(table.is_ok());
        }
    }
}
