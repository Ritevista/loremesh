//! Pure spreadsheet-like table state.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use thiserror::Error;

use crate::ViewTable;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GridError {
    #[error("CSV data is invalid: {0}")]
    Csv(String),
    #[error("table must contain at least one column")]
    NoColumns,
    #[error("column names must be non-blank and unique")]
    InvalidColumns,
    #[error("row {row} has {actual} cells; expected {expected}")]
    NonRectangular {
        row: usize,
        actual: usize,
        expected: usize,
    },
    #[error("unknown column '{0}'")]
    UnknownColumn(String),
    #[error("at least one column must remain visible")]
    NoVisibleColumns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataGrid {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    visible: Vec<usize>,
    query: String,
    filters: BTreeMap<usize, String>,
    sort: Option<(usize, SortDirection)>,
}

impl DataGrid {
    pub fn from_csv(bytes: &[u8]) -> Result<Self, GridError> {
        let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
        let columns = reader
            .headers()
            .map_err(|error| GridError::Csv(error.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let rows = reader
            .records()
            .map(|record| {
                record
                    .map(|value| value.iter().map(str::to_owned).collect::<Vec<_>>())
                    .map_err(|error| GridError::Csv(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(columns, rows)
    }

    pub fn projection_csv(&self) -> Result<Vec<u8>, GridError> {
        let table = self.projection();
        let mut writer = csv::Writer::from_writer(Vec::new());
        let safe_columns = table
            .columns
            .iter()
            .map(|column| neutralize_formula(column))
            .collect::<Vec<_>>();
        writer
            .write_record(safe_columns)
            .map_err(|error| GridError::Csv(error.to_string()))?;
        for row in &table.rows {
            let safe = row
                .iter()
                .map(|cell| neutralize_formula(cell))
                .collect::<Vec<_>>();
            writer
                .write_record(safe)
                .map_err(|error| GridError::Csv(error.to_string()))?;
        }
        writer
            .into_inner()
            .map_err(|error| GridError::Csv(error.to_string()))
    }

    pub fn new(columns: Vec<String>, rows: Vec<Vec<String>>) -> Result<Self, GridError> {
        if columns.is_empty() {
            return Err(GridError::NoColumns);
        }
        let mut normalized = columns
            .iter()
            .map(|column| column.trim().to_lowercase())
            .collect::<Vec<_>>();
        normalized.sort_unstable();
        normalized.dedup();
        if normalized.len() != columns.len() || normalized.iter().any(String::is_empty) {
            return Err(GridError::InvalidColumns);
        }
        for (row, cells) in rows.iter().enumerate() {
            if cells.len() != columns.len() {
                return Err(GridError::NonRectangular {
                    row,
                    actual: cells.len(),
                    expected: columns.len(),
                });
            }
        }
        Ok(Self {
            visible: (0..columns.len()).collect(),
            columns,
            rows,
            query: String::new(),
            filters: BTreeMap::new(),
            sort: None,
        })
    }

    pub fn total_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn matching_rows(&self) -> usize {
        self.matching_indices().len()
    }

    pub fn search(&mut self, query: impl Into<String>) {
        self.query = query.into().to_lowercase();
    }

    pub fn filter(&mut self, column: &str, value: impl Into<String>) -> Result<(), GridError> {
        let index = self.column_index(column)?;
        let value = value.into().to_lowercase();
        if value.is_empty() {
            self.filters.remove(&index);
        } else {
            self.filters.insert(index, value);
        }
        Ok(())
    }

    pub fn sort(&mut self, column: &str, direction: SortDirection) -> Result<(), GridError> {
        self.sort = Some((self.column_index(column)?, direction));
        Ok(())
    }

    pub fn show_columns(&mut self, names: &[String]) -> Result<(), GridError> {
        if names.is_empty() {
            return Err(GridError::NoVisibleColumns);
        }
        self.visible = names
            .iter()
            .map(|name| self.column_index(name))
            .collect::<Result<Vec<_>, _>>()?;
        self.visible.sort_unstable();
        self.visible.dedup();
        Ok(())
    }

    pub fn reset(&mut self) {
        self.visible = (0..self.columns.len()).collect();
        self.query.clear();
        self.filters.clear();
        self.sort = None;
    }

    pub fn projection(&self) -> ViewTable {
        let indices = self.matching_indices();
        ViewTable {
            columns: self
                .visible
                .iter()
                .map(|index| self.columns[*index].clone())
                .collect(),
            rows: indices
                .iter()
                .map(|row| {
                    self.visible
                        .iter()
                        .map(|column| self.rows[*row][*column].clone())
                        .collect()
                })
                .collect(),
        }
    }

    pub fn value_pairs(
        &self,
        label_column: &str,
        value_column: &str,
    ) -> Result<Vec<(String, String)>, GridError> {
        let label = self.column_index(label_column)?;
        let value = self.column_index(value_column)?;
        Ok(self
            .matching_indices()
            .iter()
            .map(|row| {
                (
                    self.rows[*row][label].clone(),
                    self.rows[*row][value].clone(),
                )
            })
            .collect())
    }

    fn column_index(&self, name: &str) -> Result<usize, GridError> {
        self.columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| GridError::UnknownColumn(name.into()))
    }

    fn matching_indices(&self) -> Vec<usize> {
        let mut indices =
            self.rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    (self.query.is_empty()
                        || self.visible.iter().any(|column| {
                            row[*column].to_lowercase().contains(self.query.as_str())
                        }))
                        && self.filters.iter().all(|(column, value)| {
                            row[*column].to_lowercase().contains(value.as_str())
                        })
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
        if let Some((column, direction)) = self.sort {
            indices.sort_by(|left, right| {
                let order = compare_cells(&self.rows[*left][column], &self.rows[*right][column]);
                match direction {
                    SortDirection::Ascending => order,
                    SortDirection::Descending => order.reverse(),
                }
            });
        }
        indices
    }
}

fn compare_cells(left: &str, right: &str) -> Ordering {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left_number), Ok(right_number)) => left_number.total_cmp(&right_number),
        _ => left.to_lowercase().cmp(&right.to_lowercase()),
    }
}

fn neutralize_formula(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> DataGrid {
        DataGrid::new(
            vec!["name".into(), "team".into(), "score".into()],
            vec![
                vec!["Ada".into(), "Blue".into(), "10".into()],
                vec!["Lin".into(), "Green".into(), "2".into()],
                vec!["Sam".into(), "Blue".into(), "7".into()],
            ],
        )
        .expect("valid grid")
    }

    #[test]
    fn operations_compose_without_mutating_source_rows() {
        let mut value = grid();
        value.filter("team", "blue").expect("known column");
        value.search("a");
        value
            .sort("score", SortDirection::Descending)
            .expect("known column");
        value
            .show_columns(&["name".into(), "score".into()])
            .expect("visible columns");
        assert_eq!(value.total_rows(), 3);
        assert_eq!(
            value.projection().rows,
            vec![
                vec![String::from("Ada"), String::from("10")],
                vec![String::from("Sam"), String::from("7")]
            ]
        );
    }

    #[test]
    fn invalid_shapes_and_columns_are_rejected() {
        assert!(DataGrid::new(vec!["a".into()], vec![vec![]]).is_err());
        assert!(grid().filter("missing", "x").is_err());
        assert!(grid().show_columns(&[]).is_err());
    }

    #[test]
    fn csv_is_rectangular_and_formula_safe() {
        let value = DataGrid::from_csv(b"name,value\nalpha,=1+1\n").expect("valid CSV");
        let saved =
            String::from_utf8(value.projection_csv().expect("CSV output")).expect("UTF-8 CSV");
        assert!(saved.contains("'=1+1"));
    }
}
