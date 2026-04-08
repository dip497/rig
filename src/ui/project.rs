use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

pub fn draw(app: &mut App, f: &mut Frame, area: Rect, project_idx: usize) {
    let Some(proj) = app.config.projects.get(project_idx) else {
        f.render_widget(
            Paragraph::new("Project not found").style(Style::default().fg(Color::Red)),
            area,
        );
        return;
    };

    let agents = &app.config.agents;
    let proj_dir = Some(proj.path.clone());

    // Split into skills section and MCP section
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(area);

    // ── Skills section ──────────────────────────────────
    let skills = crate::skills::scan(agents, proj_dir.as_ref());
    let mut skill_items: Vec<ListItem> = Vec::new();

    for skill in &skills {
        let mut spans = vec![
            Span::styled(
                format!("  {:<20}", skill.name),
                if skill.in_store {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ];

        for agent in agents {
            let is_on = skill.is_enabled(&agent.name);
            let (symbol, color) = if is_on {
                ("*", agent.color())
            } else {
                (".", Color::DarkGray)
            };
            let style = if is_on {
                Style::default().fg(color).bold()
            } else {
                Style::default().fg(color)
            };
            spans.push(Span::styled(
                format!("{}{} ", symbol, agent.key.to_uppercase()),
                style,
            ));
        }

        if !skill.in_store {
            spans.push(Span::styled(" !", Style::default().fg(Color::Yellow)));
        }

        skill_items.push(ListItem::new(Line::from(spans)));
    }

    if skill_items.is_empty() {
        skill_items.push(ListItem::new(Span::styled(
            "  No skills configured",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let skills_block = Block::default()
        .borders(Borders::BOTTOM)
        .title(Span::styled(
            format!(" Skills ({}) ", skills.len()),
            Style::default().fg(Color::Cyan).bold(),
        ));

    f.render_widget(List::new(skill_items).block(skills_block), sections[0]);

    // ── MCP section ─────────────────────────────────────
    let mcps = crate::mcp::scan_with_config(proj_dir.as_ref(), Some(&app.config));
    let mut mcp_items: Vec<ListItem> = Vec::new();

    for entry in &mcps {
        let status = if entry.is_disabled { "o" } else { "*" };
        let status_color = if entry.is_disabled { Color::DarkGray } else { Color::Green };
        let name_style = if entry.is_disabled {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let cmd_preview = if entry.args.is_empty() {
            entry.command.clone()
        } else {
            format!("{} ...", entry.command)
        };

        mcp_items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {} ", status), Style::default().fg(status_color)),
            Span::styled(format!("{:<20}", entry.name), name_style),
            Span::styled(
                format!("[{}] ", entry.source.label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(cmd_preview, Style::default().fg(Color::DarkGray)),
        ])));
    }

    if mcp_items.is_empty() {
        mcp_items.push(ListItem::new(Span::styled(
            "  No MCP servers for this project",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let mcp_block = Block::default()
        .title(Span::styled(
            format!(" MCP ({}) ", mcps.len()),
            Style::default().fg(Color::Green).bold(),
        ));

    f.render_widget(List::new(mcp_items).block(mcp_block), sections[1]);
}
