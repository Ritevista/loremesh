//! Semantic terminal colors shared by every workbench renderer.

use ratatui::style::{Color, Modifier, Style};

pub const PRIMARY: Color = Color::Cyan;
pub const SECONDARY: Color = Color::Magenta;
pub const SUCCESS: Color = Color::Green;
pub const WARNING: Color = Color::Yellow;
pub const DANGER: Color = Color::Red;
pub const MUTED: Color = Color::DarkGray;
pub const TEXT: Color = Color::Gray;
pub const FOCUS: Color = Color::LightCyan;
pub const SURFACE_ALT: Color = Color::Rgb(24, 28, 38);

const SERIES: [Color; 6] = [PRIMARY, SECONDARY, SUCCESS, WARNING, Color::Blue, DANGER];

pub fn series(index: usize) -> Color {
    SERIES[index % SERIES.len()]
}

pub fn focused() -> Style {
    Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(FOCUS)
        .add_modifier(Modifier::BOLD)
}

pub fn header() -> Style {
    Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn value(value: &str) -> Style {
    let color = match value.trim().to_ascii_lowercase().as_str() {
        "ok" | "passed" | "connected" | "verified" | "success" => SUCCESS,
        "critical" | "failed" | "rejected" | "error" | "disconnected" => DANGER,
        "warning" | "waiting" | "stale" | "degraded" => WARNING,
        "disputed" | "inferred" => SECONDARY,
        "unreviewed" | "unknown" | "disabled" => MUTED,
        _ => TEXT,
    };
    Style::default().fg(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_values_use_stable_roles() {
        assert_eq!(value("verified").fg, Some(SUCCESS));
        assert_eq!(value("failed").fg, Some(DANGER));
        assert_eq!(value("stale").fg, Some(WARNING));
        assert_eq!(value("disputed").fg, Some(SECONDARY));
        assert_eq!(value("ordinary text").fg, Some(TEXT));
        assert_eq!(selected().bg, Some(FOCUS));
    }

    #[test]
    fn series_palette_is_stable_and_cycles() {
        assert_eq!(series(0), PRIMARY);
        assert_eq!(series(1), SECONDARY);
        assert_eq!(series(6), PRIMARY);
    }
}
