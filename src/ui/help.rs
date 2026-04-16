use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

pub fn draw(_app: &App, f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            " Keyboard Reference",
            Style::default().fg(Color::White).bold(),
        )),
        Line::from(""),
        section("Navigation"),
        key_line("j / k / Up / Down", "Move cursor up/down"),
        key_line("h / l / Left / Right", "Move cursor left/right (matrix columns)"),
        key_line("g / G", "Jump to top / bottom"),
        key_line("> / <", "Scroll project columns right / left"),
        Line::from(""),
        section("Screens"),
        key_line("s", "Skills matrix view"),
        key_line("m", "MCP servers view"),
        key_line("p / Enter", "Project detail view"),
        key_line("Esc", "Go back / close"),
        key_line("?", "Toggle this help"),
        Line::from(""),
        section("Skills (Matrix view)"),
        key_line("Space", "Toggle skill for current column's project"),
        key_line("Enter", "Show skill detail overlay"),
        key_line("1-9", "Toggle skill for agent N"),
        key_line("E / D", "Bulk enable / disable all skills"),
        key_line("u", "Uninstall skill (removes from store + all agents)"),
        key_line("S", "Cycle sort: name → enabled count"),
        key_line("i", "Install a skill (type source URL, Enter to install)"),
        Line::from(""),
        section("MCP"),
        key_line("x", "Toggle MCP enabled/disabled (soft, non-destructive)"),
        key_line("d", "Delete MCP entry from config file (permanent)"),
        Line::from(""),
        section("Project Detail"),
        key_line("n / b", "Next / previous project"),
        key_line("Space", "Toggle skill for selected agent"),
        Line::from(""),
        section("Other"),
        key_line("/", "Filter / search (Esc to clear, Enter to confirm)"),
        key_line("  ↑/↓  or  Ctrl+j/k", "Navigate results while filtering"),
        key_line("\\", "Clear active filter"),
        key_line("i", "Install a skill from GitHub or local path"),
        key_line("r", "Rescan / refresh"),
        key_line("q", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc or ? to close this help",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn section(title: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" -- {} ", title),
            Style::default().fg(Color::Cyan).bold(),
        ),
    ])
}

fn key_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("   {:<28}", key),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}
