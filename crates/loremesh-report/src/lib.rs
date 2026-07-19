//! Renderer-independent `LoreMesh` report models and deterministic exporters.
#![forbid(unsafe_code)]

use std::fmt::Write as _;

use loremesh_core::{
    investigation::{EvidenceStatus, Investigation, InvestigationItem},
    relationship::{CodeReference, Relationship, RelationshipEndpoint},
    Artifact, Feedback, FeedbackTarget, Finding, ReportId, Source, SourceSnapshot,
};
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

/// Resolved source lineage safe for portable report presentation.
#[derive(Debug, Clone)]
pub struct InvestigationLineage {
    pub artifact: Artifact,
    pub snapshot: SourceSnapshot,
    pub source: Source,
    pub evidence_label: Option<String>,
    pub evidence_status: EvidenceStatus,
}

/// Canonical objects resolved by the application for deterministic reporting.
pub struct InvestigationReportInput<'a> {
    pub investigation: &'a Investigation,
    pub artifacts: &'a [Artifact],
    pub findings: &'a [Finding],
    pub relationships: &'a [Relationship],
    pub code_references: &'a [CodeReference],
    pub feedback: &'a [Feedback],
    pub lineage: &'a [InvestigationLineage],
}

/// Builds the shared structured report model for one investigation.
pub struct InvestigationReportBuilder;

impl InvestigationReportBuilder {
    #[allow(clippy::too_many_lines)]
    pub fn build(input: &InvestigationReportInput<'_>) -> Result<Report, ReportError> {
        let investigation = input.investigation;
        let claim_count = input
            .findings
            .iter()
            .map(|finding| finding.claims.len())
            .sum::<usize>();
        let evidence_count = input
            .findings
            .iter()
            .flat_map(|finding| &finding.claims)
            .map(|claim| claim.evidence.len())
            .sum::<usize>();
        let metadata = ReportSection::new(
            "Investigation metadata",
            vec![
                ReportBlock::Metric(Metric::new(
                    "Status",
                    label_debug(investigation.status),
                    None,
                )?),
                ReportBlock::Metric(Metric::new(
                    "Scope",
                    label_debug(investigation.scope),
                    None,
                )?),
                ReportBlock::Paragraph(investigation.description.clone()),
            ],
        )?;
        let coverage = ReportSection::new(
            "Knowledge coverage",
            vec![
                metric("Artifacts", input.artifacts.len())?,
                metric("Findings", input.findings.len())?,
                metric("Claims", claim_count)?,
                metric("Relationships", input.relationships.len())?,
                metric("Evidence", evidence_count)?,
            ],
        )?;
        let artifacts = ReportSection::new(
            "Collected artifacts",
            vec![ReportBlock::Table(TableModel::new(
                "Artifacts",
                vec!["Artifact ID".into(), "Name".into(), "Snapshot ID".into()],
                input
                    .artifacts
                    .iter()
                    .map(|artifact| {
                        vec![
                            artifact.id.to_string(),
                            artifact.name.clone(),
                            artifact.snapshot_id.to_string(),
                        ]
                    })
                    .collect(),
            )?)],
        )?;
        let findings = ReportSection::new(
            "Findings and claims",
            vec![ReportBlock::Table(TableModel::new(
                "Findings",
                vec![
                    "Finding".into(),
                    "Status".into(),
                    "Claim".into(),
                    "Evidence".into(),
                ],
                input
                    .findings
                    .iter()
                    .flat_map(|finding| {
                        finding.claims.iter().map(move |claim| {
                            vec![
                                finding.title.clone(),
                                label_debug(finding.status),
                                claim.text.clone(),
                                claim
                                    .evidence
                                    .iter()
                                    .map(|evidence| evidence.label.clone())
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            ]
                        })
                    })
                    .collect(),
            )?)],
        )?;
        let lineage = ReportSection::new(
            "Source lineage",
            vec![ReportBlock::Table(TableModel::new(
                "Lineage",
                vec![
                    "Evidence".into(),
                    "Status".into(),
                    "Artifact".into(),
                    "Snapshot".into(),
                    "Source".into(),
                ],
                input
                    .lineage
                    .iter()
                    .map(|lineage| {
                        vec![
                            lineage.evidence_label.clone().unwrap_or_default(),
                            label_debug(lineage.evidence_status),
                            lineage.artifact.name.clone(),
                            lineage.snapshot.id.to_string(),
                            lineage.source.location.clone(),
                        ]
                    })
                    .collect(),
            )?)],
        )?;
        let relationships = ReportSection::new(
            "Relationship provenance",
            vec![ReportBlock::Table(TableModel::new(
                "Relationships",
                vec![
                    "Source".into(),
                    "Relationship".into(),
                    "Target".into(),
                    "Origin".into(),
                    "Status".into(),
                    "Engine".into(),
                ],
                input
                    .relationships
                    .iter()
                    .map(|relationship| {
                        vec![
                            endpoint(&relationship.source),
                            relationship.relation.as_str().into(),
                            endpoint(&relationship.target),
                            label_debug(relationship.origin),
                            label_debug(relationship.status),
                            relationship
                                .external_provenance
                                .as_ref()
                                .map_or_else(String::new, |value| value.provider.clone()),
                        ]
                    })
                    .collect(),
            )?)],
        )?;
        let review = ReportSection::new(
            "Review and feedback",
            vec![
                ReportBlock::Table(TableModel::new(
                    "Investigation notes",
                    vec!["Scope".into(), "Note".into()],
                    investigation
                        .notes
                        .iter()
                        .map(|note| vec![label_debug(investigation.scope), note.text.clone()])
                        .collect(),
                )?),
                ReportBlock::Table(TableModel::new(
                    "Canonical feedback",
                    vec![
                        "Target".into(),
                        "Scope".into(),
                        "Status".into(),
                        "Text".into(),
                    ],
                    input
                        .feedback
                        .iter()
                        .filter(|feedback| feedback_is_collected(feedback, investigation))
                        .map(|feedback| {
                            vec![
                                feedback_target(&feedback.target),
                                label_debug(feedback.scope),
                                label_debug(feedback.status),
                                feedback.text.clone(),
                            ]
                        })
                        .collect(),
                )?),
            ],
        )?;
        let code = ReportSection::new(
            "Code references",
            vec![ReportBlock::Table(TableModel::new(
                "Code references",
                vec!["Repository".into(), "Revision".into(), "Path".into()],
                input
                    .code_references
                    .iter()
                    .map(|code| {
                        vec![
                            code.repository.clone(),
                            code.revision.clone(),
                            code.path.clone(),
                        ]
                    })
                    .collect(),
            )?)],
        )?;
        Report::new(
            ReportId::deterministic(investigation.id.as_str()),
            investigation.title.clone(),
            vec![
                metadata,
                coverage,
                artifacts,
                findings,
                lineage,
                relationships,
                review,
                code,
            ],
        )
    }
}

fn label_debug(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn metric(label: &str, value: usize) -> Result<ReportBlock, ReportError> {
    Ok(ReportBlock::Metric(Metric::new(
        label,
        value.to_string(),
        None,
    )?))
}

fn endpoint(value: &RelationshipEndpoint) -> String {
    match value {
        RelationshipEndpoint::Artifact(id) => format!("Artifact {id}"),
        RelationshipEndpoint::Source(id) => format!("Source {id}"),
        RelationshipEndpoint::Snapshot(id) => format!("Snapshot {id}"),
        RelationshipEndpoint::Code(id) => format!("Code {id}"),
    }
}

fn feedback_target(value: &FeedbackTarget) -> String {
    match value {
        FeedbackTarget::Artifact(id) => id.to_string(),
        FeedbackTarget::Finding(id) => id.to_string(),
        FeedbackTarget::Claim(id) => id.to_string(),
        FeedbackTarget::TraceEdge(id) => id.to_string(),
        FeedbackTarget::Relationship(id) => id.to_string(),
    }
}

fn feedback_is_collected(feedback: &Feedback, investigation: &Investigation) -> bool {
    investigation.items.iter().any(|item| {
        matches!((item, &feedback.target),
            (InvestigationItem::Artifact(left), FeedbackTarget::Artifact(right)) if left == right)
            || matches!((item, &feedback.target),
                (InvestigationItem::Finding(left), FeedbackTarget::Finding(right)) if left == right)
            || matches!((item, &feedback.target),
                (InvestigationItem::Claim(left), FeedbackTarget::Claim(right)) if left == right)
            || matches!((item, &feedback.target),
                (InvestigationItem::Relationship(left), FeedbackTarget::Relationship(right)) if left == right)
    })
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
    use loremesh_core::investigation::{InvestigationScope, InvestigationStatus};
    use loremesh_core::relationship::{ExternalProvenance, RelationType};
    use loremesh_core::{
        ArtifactId, ArtifactReference, Claim, ClaimId, EdgeOrigin, EvidenceReference, FindingId,
        KnowledgeScope, RelationshipId, SnapshotId, SourceId, VerificationStatus,
    };
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
    fn investigation_html_is_deterministic_escaped_and_contains_lineage_and_provenance() {
        let source =
            Source::local(SourceId::deterministic("source"), "docs/alpha.md").expect("source");
        let snapshot = SourceSnapshot::new(
            SnapshotId::deterministic("snapshot"),
            source.id.clone(),
            "a".repeat(64),
            14,
        )
        .expect("snapshot");
        let artifact = Artifact::new(
            ArtifactId::deterministic("artifact"),
            snapshot.id.clone(),
            "<script>alert(1)</script>",
            14,
        )
        .expect("artifact");
        let evidence = EvidenceReference::new(
            ArtifactReference {
                artifact_id: artifact.id.clone(),
            },
            0,
            8,
            "section 4.2",
            "evidence bytes",
        )
        .expect("evidence");
        let finding = Finding::new(
            FindingId::deterministic("finding"),
            "Interface confirmed",
            KnowledgeScope::Personal,
            VerificationStatus::Verified,
            vec![Claim::new(
                ClaimId::deterministic("claim"),
                "Protocol Y is used.",
                vec![evidence],
            )
            .expect("claim")],
        )
        .expect("finding");
        let relationship = Relationship::new(
            RelationshipEndpoint::Artifact(artifact.id.clone()),
            RelationType::parse("depends_on").expect("relation"),
            RelationshipEndpoint::Source(source.id.clone()),
            EdgeOrigin::Extracted,
            VerificationStatus::Unreviewed,
            Vec::new(),
            Some(ExternalProvenance {
                provider: "graphify".into(),
                provider_version: "1".into(),
                run_id: "offline-import".into(),
                configuration_digest: "b".repeat(64),
                observed_at: None,
                external_id: Some("edge-1".into()),
            }),
        )
        .expect("relationship");
        let mut investigation = Investigation::new(
            loremesh_core::InvestigationId::deterministic("investigation"),
            "Feature Alpha",
            "Deterministic report",
            InvestigationScope::Personal,
        )
        .expect("investigation");
        investigation.add_item(InvestigationItem::Artifact(artifact.id.clone()));
        investigation.add_item(InvestigationItem::Finding(finding.id.clone()));
        investigation.add_item(InvestigationItem::Relationship(
            RelationshipId::parse(relationship.id.to_string()).expect("relationship id"),
        ));
        investigation
            .transition_to(InvestigationStatus::InReview)
            .expect("transition");
        let lineage = InvestigationLineage {
            artifact: artifact.clone(),
            snapshot,
            source,
            evidence_label: Some("section 4.2".into()),
            evidence_status: EvidenceStatus::Historical,
        };
        let input = InvestigationReportInput {
            investigation: &investigation,
            artifacts: &[artifact],
            findings: &[finding],
            relationships: &[relationship],
            code_references: &[],
            feedback: &[],
            lineage: &[lineage],
        };
        let report = InvestigationReportBuilder::build(&input).expect("report");
        let first = render_html(&report);
        let second = render_html(&report);
        assert_eq!(first, second);
        assert!(first.contains("Historical"));
        assert!(first.contains("graphify"));
        assert!(first.contains("Protocol Y is used."));
        assert!(first.contains("docs/alpha.md"));
        assert!(first.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!first.contains("<script>"));
        assert!(!first.contains("/home/"));
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
