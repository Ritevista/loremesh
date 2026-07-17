//! Renderer-neutral chart data and deterministic text rendering.

use std::fmt::Write as _;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKind {
    Bar,
    HorizontalBar,
    Line,
    Pie,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartValue {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartModel {
    pub title: String,
    pub kind: ChartKind,
    pub values: Vec<ChartValue>,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ChartError {
    #[error("chart title and labels must be non-blank")]
    BlankLabel,
    #[error("chart requires at least one value")]
    NoValues,
    #[error("chart values must be finite numbers")]
    NonFinite,
    #[error("'{value}' is not a finite number")]
    InvalidNumber { value: String },
}

impl ChartModel {
    pub fn new(
        title: impl Into<String>,
        kind: ChartKind,
        values: Vec<ChartValue>,
    ) -> Result<Self, ChartError> {
        let title = title.into();
        if title.trim().is_empty() || values.iter().any(|value| value.label.trim().is_empty()) {
            return Err(ChartError::BlankLabel);
        }
        if values.is_empty() {
            return Err(ChartError::NoValues);
        }
        if values.iter().any(|value| !value.value.is_finite()) {
            return Err(ChartError::NonFinite);
        }
        Ok(Self {
            title,
            kind,
            values,
        })
    }

    pub fn from_pairs(
        title: impl Into<String>,
        kind: ChartKind,
        pairs: Vec<(String, String)>,
    ) -> Result<Self, ChartError> {
        let values = pairs
            .into_iter()
            .map(|(label, raw)| {
                let value = raw
                    .parse::<f64>()
                    .map_err(|_| ChartError::InvalidNumber { value: raw.clone() })?;
                Ok(ChartValue { label, value })
            })
            .collect::<Result<Vec<_>, ChartError>>()?;
        Self::new(title, kind, values)
    }

    pub fn render_text(&self, width: usize) -> String {
        match self.kind {
            ChartKind::Bar | ChartKind::HorizontalBar => self.render_bars(width),
            ChartKind::Line => self.render_line(),
            ChartKind::Pie => self.render_pie(),
        }
    }

    fn render_bars(&self, width: usize) -> String {
        let maximum = self
            .values
            .iter()
            .map(|value| value.value.abs())
            .fold(0.0_f64, f64::max);
        let bar_width = width.saturating_sub(24).clamp(1, 60);
        let bar_width_u32 = u32::try_from(bar_width).map_or(60, std::convert::identity);
        let mut rendered = String::new();
        for value in &self.values {
            let cells = if maximum == 0.0 {
                0
            } else {
                let ratio = value.value.abs() / maximum;
                (1..=bar_width_u32)
                    .filter(|cell| f64::from(*cell) / f64::from(bar_width_u32) <= ratio)
                    .count()
            };
            let _ = writeln!(
                rendered,
                "{:<16} {:>8.2} {}",
                truncate(&value.label, 16),
                value.value,
                "█".repeat(cells)
            );
        }
        rendered
    }

    fn render_line(&self) -> String {
        let mut rendered = String::new();
        for (index, value) in self.values.iter().enumerate() {
            let connector = if index == 0 { "●" } else { "─●" };
            let _ = write!(
                rendered,
                "{connector} {} ({:.2}) ",
                value.label, value.value
            );
        }
        rendered
    }

    fn render_pie(&self) -> String {
        let total = self
            .values
            .iter()
            .map(|value| value.value.abs())
            .sum::<f64>();
        let mut rendered = String::new();
        for value in &self.values {
            let percent = if total == 0.0 {
                0.0
            } else {
                value.value.abs() * 100.0 / total
            };
            let _ = writeln!(
                rendered,
                "◉ {:<16} {:>6.1}%",
                truncate(&value.label, 16),
                percent
            );
        }
        rendered
    }
}

fn truncate(value: &str, length: usize) -> String {
    value.chars().take(length).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chart_kind_has_a_deterministic_text_view() {
        for kind in [
            ChartKind::Bar,
            ChartKind::HorizontalBar,
            ChartKind::Line,
            ChartKind::Pie,
        ] {
            let chart = ChartModel::new(
                "Scores",
                kind,
                vec![
                    ChartValue {
                        label: "Alpha".into(),
                        value: 2.0,
                    },
                    ChartValue {
                        label: "Beta".into(),
                        value: 1.0,
                    },
                ],
            )
            .expect("valid chart");
            let rendered = chart.render_text(80);
            assert!(rendered.contains("Alpha"));
            assert!(rendered.contains("Beta"));
        }
    }

    #[test]
    fn non_numeric_and_non_finite_values_are_rejected() {
        assert!(
            ChartModel::from_pairs("chart", ChartKind::Bar, vec![("x".into(), "nope".into())])
                .is_err()
        );
        assert!(ChartModel::new(
            "chart",
            ChartKind::Bar,
            vec![ChartValue {
                label: "x".into(),
                value: f64::NAN
            }]
        )
        .is_err());
    }
}
