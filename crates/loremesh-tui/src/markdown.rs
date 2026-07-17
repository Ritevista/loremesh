//! Small offline Markdown presentation model with diagram-source preservation.

use crate::browser::neutralize_terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    Mermaid,
    D2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    Heading { level: usize, text: String },
    Paragraph(String),
    ListItem(String),
    Quote(String),
    Code { language: String, source: String },
    Diagram { kind: DiagramKind, source: String },
    ThematicBreak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    pub blocks: Vec<MarkdownBlock>,
}

impl MarkdownDocument {
    pub fn parse(source: &str) -> Self {
        let safe = neutralize_terminal(source);
        let mut blocks = Vec::new();
        let mut lines = safe.lines().peekable();
        while let Some(line) = lines.next() {
            if let Some(language) = line.strip_prefix("```") {
                let mut body = Vec::new();
                for code_line in lines.by_ref() {
                    if code_line == "```" {
                        break;
                    }
                    body.push(code_line);
                }
                let source = body.join("\n");
                match language.trim() {
                    "mermaid" => blocks.push(MarkdownBlock::Diagram {
                        kind: DiagramKind::Mermaid,
                        source,
                    }),
                    "d2" => blocks.push(MarkdownBlock::Diagram {
                        kind: DiagramKind::D2,
                        source,
                    }),
                    value => blocks.push(MarkdownBlock::Code {
                        language: value.into(),
                        source,
                    }),
                }
            } else if let Some(text) = heading(line) {
                blocks.push(MarkdownBlock::Heading {
                    level: text.0,
                    text: text.1.into(),
                });
            } else if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                blocks.push(MarkdownBlock::ListItem(text.into()));
            } else if let Some(text) = line.strip_prefix("> ") {
                blocks.push(MarkdownBlock::Quote(text.into()));
            } else if matches!(line.trim(), "---" | "***") {
                blocks.push(MarkdownBlock::ThematicBreak);
            } else if !line.trim().is_empty() {
                blocks.push(MarkdownBlock::Paragraph(inert_html(line)));
            }
        }
        Self { blocks }
    }

    pub fn render_text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| match block {
                MarkdownBlock::Heading { level, text } => {
                    format!("{} {text}", "#".repeat(*level))
                }
                MarkdownBlock::Paragraph(text) => text.clone(),
                MarkdownBlock::ListItem(text) => format!("• {text}"),
                MarkdownBlock::Quote(text) => format!("│ {text}"),
                MarkdownBlock::Code { language, source } => {
                    format!("[{language} code]\n{source}")
                }
                MarkdownBlock::Diagram { kind, source } => {
                    let diagram = render_diagram(*kind, source)
                        .unwrap_or_else(|| "Unsupported diagram syntax; showing source.".into());
                    format!("[{kind:?} diagram]\n{diagram}\n\n[source]\n{source}")
                }
                MarkdownBlock::ThematicBreak => "─".repeat(40),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn render_diagram(kind: DiagramKind, source: &str) -> Option<String> {
    let arrow = match kind {
        DiagramKind::Mermaid => "-->",
        DiagramKind::D2 => "->",
    };
    let edges = source
        .lines()
        .filter_map(|line| {
            let (left, right) = line.trim().split_once(arrow)?;
            let left = clean_node(left);
            let right = clean_node(right);
            (!left.is_empty() && !right.is_empty()).then(|| format!("{left} ──▶ {right}"))
        })
        .collect::<Vec<_>>();
    (!edges.is_empty()).then(|| edges.join("\n"))
}

fn clean_node(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character: char| matches!(character, '[' | ']' | '(' | ')' | '"'))
        .split(['[', '('])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (level > 0 && level <= 6 && line.as_bytes().get(level) == Some(&b' '))
        .then(|| (level, line[level + 1..].trim()))
}

fn inert_html(value: &str) -> String {
    value.replace('<', "‹").replace('>', "›")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_blocks_and_diagram_sources_are_preserved() {
        let document =
            MarkdownDocument::parse("# Title\n\n- item\n\n```mermaid\ngraph TD\n  A --> B\n```\n");
        assert!(matches!(document.blocks[0], MarkdownBlock::Heading { .. }));
        assert!(matches!(
            document.blocks[2],
            MarkdownBlock::Diagram {
                kind: DiagramKind::Mermaid,
                ..
            }
        ));
        assert!(document.render_text().contains("A --> B"));
        assert!(document.render_text().contains("A ──▶ B"));
    }

    #[test]
    fn raw_html_and_terminal_controls_are_inert() {
        let rendered = MarkdownDocument::parse("<script>bad</script>\n\x1b[31m").render_text();
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains('\x1b'));
    }
}
