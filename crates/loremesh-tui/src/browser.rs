//! Pure models for safe code-browser presentation.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEntry {
    pub relative_path: String,
    pub is_directory: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeDocument {
    pub relative_path: String,
    lines: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BrowserError {
    #[error("path must be workspace-relative")]
    UnsafePath,
    #[error("file is not valid UTF-8")]
    InvalidUtf8,
    #[error("file contains binary NUL bytes")]
    Binary,
    #[error("file exceeds the {limit} byte viewing limit")]
    TooLarge { limit: usize },
}

impl CodeDocument {
    pub fn from_bytes(
        relative_path: impl Into<String>,
        bytes: &[u8],
        limit: usize,
    ) -> Result<Self, BrowserError> {
        if bytes.len() > limit {
            return Err(BrowserError::TooLarge { limit });
        }
        if bytes.contains(&0) {
            return Err(BrowserError::Binary);
        }
        let source = std::str::from_utf8(bytes).map_err(|_| BrowserError::InvalidUtf8)?;
        Ok(Self {
            relative_path: relative_path.into(),
            lines: source.lines().map(neutralize_terminal).collect(),
        })
    }

    pub fn numbered_text(&self) -> String {
        let width = self.lines.len().max(1).to_string().len();
        self.lines
            .iter()
            .enumerate()
            .map(|(index, line)| format!("{:>width$} │ {line}", index + 1))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = if case_sensitive {
            query.into()
        } else {
            query.to_lowercase()
        };
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(line_index, line)| {
                let haystack = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
                haystack.find(&needle).map(|column| SearchMatch {
                    line: line_index + 1,
                    column: column + 1,
                })
            })
            .collect()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

pub fn neutralize_terminal(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if matches!(character, '\t' | '\n') || !character.is_control() {
                character
            } else {
                '�'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_stable_line_numbers_and_search() {
        let document = CodeDocument::from_bytes("src/lib.rs", b"fn main() {}\n// Main\n", 100)
            .expect("text document");
        assert_eq!(document.line_count(), 2);
        assert!(document.numbered_text().starts_with("1 │ fn main"));
        assert_eq!(document.search("main", false).len(), 2);
        assert_eq!(document.search("main", true).len(), 1);
    }

    #[test]
    fn binary_large_and_control_content_is_safe() {
        assert_eq!(
            CodeDocument::from_bytes("x", b"a\0b", 100),
            Err(BrowserError::Binary)
        );
        assert!(matches!(
            CodeDocument::from_bytes("x", b"long", 2),
            Err(BrowserError::TooLarge { .. })
        ));
        let safe = CodeDocument::from_bytes("x", b"a\x1bb", 100).expect("document");
        assert!(!safe.numbered_text().contains('\x1b'));
    }
}
