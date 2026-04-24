use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::app::{App, Focus};

pub fn draw(app: &mut App, f: &mut Frame, area: Rect) {
    let filtered = app.filtered_mcp();
    if filtered.is_empty() {
        let msg = if app.mcp_entries.is_empty() {
            "No MCP servers found. Add to ~/.mcp.json or project .mcp.json"
        } else {
            "No MCP servers match the current filter"
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    // Group entries by source label
    let mut groups: Vec<(&str, Vec<(usize, &crate::mcp::McpEntry)>)> = Vec::new();
    for (idx, entry) in filtered.iter().enumerate() {
        let label = entry.source.label.as_str();
        if let Some(group) = groups.iter_mut().find(|(l, _)| *l == label) {
            group.1.push((idx, entry));
        } else {
            groups.push((label, vec![(idx, entry)]));
        }
    }

    let mut items: Vec<ListItem> = Vec::new();
    let mut item_to_filtered_idx: Vec<Option<usize>> = Vec::new(); // for mapping selection

    for (label, entries) in &groups {
        // Group header
        let source_color = match *label {
            l if l.contains("Global") => Color::Green,
            l if l.contains("Project") => Color::Cyan,
            l if l.contains("GSD") => Color::Magenta,
            _ => Color::White,
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!(" -- {} ", label),
                Style::default().fg(source_color).bold(),
            ),
            Span::styled(
                format!("({}) ", entries.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ])));
        item_to_filtered_idx.push(None); // headers aren't selectable

        for (idx, entry) in entries {
            let status_symbol = if entry.is_disabled { "o" } else { "*" };
            let status_color = if entry.is_disabled { Color::DarkGray } else { Color::Green };

            let cmd = if entry.args.is_empty() {
                entry.command.clone()
            } else {
                format!("{} {}", entry.command, truncate_cmd(&entry.args.join(" "), 40))
            };

            let name_style = if entry.is_disabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", status_symbol), Style::default().fg(status_color)),
                Span::styled(format!("{:<22}", entry.name), name_style),
                Span::styled(cmd, Style::default().fg(Color::DarkGray)),
            ])));
            item_to_filtered_idx.push(Some(*idx));
        }
    }

    let hl = if app.focus == Focus::Content {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    f.render_stateful_widget(
        List::new(items).highlight_style(hl),
        area,
        &mut app.list_state,
    );
}

fn truncate_cmd(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
