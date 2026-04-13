use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::skills;

pub fn draw(_app: &App, f: &mut Frame, skill_name: &str) {
    let area = centered_rect(60, 18, f.area());

    // Clear the background area
    f.render_widget(Clear, area);

    let detail = skills::load_detail(skill_name);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(ref d) = detail {
        if !d.description.is_empty() {
            lines.push(row("Description", &d.description, Color::White));
        }
        if !d.version.is_empty() && d.version != "—" {
            lines.push(row("Version", &d.version, Color::Cyan));
        }
        if !d.creator.is_empty() {
            lines.push(row("Creator", &d.creator, Color::Yellow));
        }
        if !d.license.is_empty() {
            lines.push(row("License", &d.license, Color::DarkGray));
        }
        if !d.compatibility.is_empty() {
            lines.push(row("Compat", &d.compatibility, Color::Green));
        }
        if !d.effort.is_empty() {
            lines.push(row("Effort", &d.effort, Color::Magenta));
        }
        lines.push(Line::from(""));
        lines.push(row("Files", &d.file_count.to_string(), Color::White));
        if d.is_symlink {
            let target = d.symlink_target
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "?".into());
            lines.push(row("Symlink →", &target, Color::Cyan));
        }
        if let Some(ref src) = d.source {
            lines.push(row("Source", src, Color::Yellow));
        }
        if let Some(ref commit) = d.commit {
            lines.push(row("Commit", commit, Color::DarkGray));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {}", d.store_path.display()),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Not in skill store",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let title = format!(" {} ", skill_name);
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::White).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn row(label: &str, value: &str, val_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<14}", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value.to_string(), Style::default().fg(val_color)),
    ])
}

/// Returns a centered rectangle of the given width (chars) and height (rows),
/// clamped to fit within `r`.
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width.saturating_sub(4));
    let h = height.min(r.height.saturating_sub(2));
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}
