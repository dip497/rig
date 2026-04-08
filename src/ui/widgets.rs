use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, Focus, Mode, Screen, StatusKind};

// ── Sidebar ────────────────────────────────────────────────────────────────

pub fn draw_sidebar(app: &mut App, f: &mut Frame, area: Rect) {
    let agents = &app.config.agents;

    let items: Vec<ListItem> = std::iter::once(
        ListItem::new(Line::from(vec![
            Span::styled(" * ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("Global", Style::default().fg(Color::Yellow).bold()),
        ])),
    )
    .chain(app.config.projects.iter().map(|proj| {
        let mut badges = vec![Span::raw("   ")];
        for agent in agents.iter() {
            if agent.has_signal_in(&proj.path) {
                badges.push(Span::styled(
                    agent.key.to_uppercase().to_string(),
                    Style::default().fg(agent.color()).bold(),
                ));
            } else {
                badges.push(Span::styled(".", Style::default().fg(Color::DarkGray)));
            }
        }
        badges.push(Span::styled(
            format!(" {}", proj.name),
            Style::default().fg(Color::Cyan),
        ));
        ListItem::new(Line::from(badges))
    }))
    .collect();

    let highlight = if app.focus == Focus::Sidebar {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    } else {
        Style::default()
    };

    let sidebar = List::new(items)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(Span::styled(" rig ", Style::default().fg(Color::White).bold())),
        )
        .highlight_style(highlight);
    f.render_stateful_widget(sidebar, area, &mut app.sidebar_state);
}

// ── Header ─────────────────────────────────────────────────────────────────

pub fn draw_header(app: &App, f: &mut Frame, area: Rect) {
    let (title, path) = match app.project_idx {
        None => (
            "Global".into(),
            dirs::home_dir()
                .map(|h| h.display().to_string())
                .unwrap_or_default(),
        ),
        Some(i) => {
            let proj = &app.config.projects[i];
            (proj.name.clone(), proj.path.display().to_string())
        }
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, Style::default().fg(Color::White).bold()),
            Span::raw("  "),
            Span::styled(path, Style::default().fg(Color::DarkGray)),
        ])),
        area,
    );
}

// ── Screen tabs ────────────────────────────────────────────────────────────

pub fn draw_screen_tabs(app: &App, f: &mut Frame, area: Rect) {
    let en = app.skills.iter().filter(|s| s.any_enabled()).count();
    let total = app.skills.len();
    let mcp_n = app.mcp_entries.len();

    let skill_active = matches!(app.screen, Screen::Matrix);
    let mcp_active = matches!(app.screen, Screen::Mcp);
    let proj_active = matches!(app.screen, Screen::ProjectDetail(_));

    let mut spans = vec![
        tab_span("s", "kills", &format!(" [{}/{}]", en, total), skill_active, Color::Cyan),
        Span::raw("  "),
        tab_span("m", "cp", &format!(" [{}]", mcp_n), mcp_active, Color::Green),
    ];

    if proj_active {
        spans.push(Span::raw("  "));
        spans.push(tab_span("p", "roject", "", true, Color::Magenta));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn tab_span(key: &str, rest: &str, suffix: &str, active: bool, color: Color) -> Span<'static> {
    let fg = if active { color } else { Color::DarkGray };
    Span::styled(
        format!("[{}]{}{}", key, rest, suffix),
        Style::default().fg(fg).bold(),
    )
}

// ── Filter bar ─────────────────────────────────────────────────────────────

pub fn draw_filter_bar(app: &App, f: &mut Frame, area: Rect) {
    match &app.mode {
        Mode::Filter(q) => {
            let text = format!("/{}", q);
            f.render_widget(
                Paragraph::new(text)
                    .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray)),
                area,
            );
        }
        _ => {
            // Show active filter if any (from last confirmed search)
            // Currently filters are cleared on mode exit, so nothing here
        }
    }
}

// ── Status bar ─────────────────────────────────────────────────────────────

pub fn draw_status(app: &App, f: &mut Frame, area: Rect) {
    let Some(ref status) = app.status else { return };
    if status.is_expired() {
        return;
    }

    let col = match status.kind {
        StatusKind::Ok => Color::Green,
        StatusKind::Err => Color::Red,
        StatusKind::Info => Color::Yellow,
    };

    f.render_widget(
        Paragraph::new(Span::styled(&status.msg, Style::default().fg(col))),
        area,
    );
}

// ── Help bar ───────────────────────────────────────────────────────────────

pub fn draw_help_bar(app: &App, f: &mut Frame, area: Rect) {
    let line1 = match &app.screen {
        Screen::Matrix => matrix_help(app),
        Screen::Mcp => mcp_help(),
        Screen::ProjectDetail(_) => project_help(app),
        Screen::Help => vec![
            help_key("Esc", "back"),
        ],
    };

    let line2 = vec![
        help_key("h/l", "cols"),
        help_key("j/k", "rows"),
        help_key("/", "filter"),
        help_key("?", "help"),
        help_key("q", "quit"),
    ];

    f.render_widget(
        Paragraph::new(vec![Line::from(line1), Line::from(line2)]),
        area,
    );
}

fn matrix_help(app: &App) -> Vec<Span<'static>> {
    let mut spans = vec![
        help_key("Space", "toggle"),
    ];

    // Show agent column numbers
    for (i, agent) in app.config.agents.iter().enumerate() {
        spans.push(Span::styled(
            format!("  {}", i + 1),
            Style::default().fg(agent.color()).bold(),
        ));
        spans.push(Span::styled(
            format!(" {}", agent.name),
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans.push(Span::raw("  "));
    spans.push(help_key("E/D", "bulk"));
    spans
}

fn mcp_help() -> Vec<Span<'static>> {
    vec![
        help_key("x", "toggle"),
        help_key("d", "delete"),
        help_key("Enter", "detail"),
    ]
}

fn project_help(app: &App) -> Vec<Span<'static>> {
    let mut spans = vec![
        help_key("Space", "toggle"),
    ];
    for (i, agent) in app.config.agents.iter().enumerate() {
        spans.push(Span::styled(
            format!("  {}", i + 1),
            Style::default().fg(agent.color()).bold(),
        ));
        spans.push(Span::styled(
            format!(" {}", agent.name),
            Style::default().fg(Color::DarkGray),
        ));
    }
    spans.push(Span::raw("  "));
    spans.push(help_key("n/b", "next/prev"));
    spans
}

fn help_key(key: &str, label: &str) -> Span<'static> {
    Span::styled(
        format!(" {}:{} ", key, label),
        Style::default().fg(Color::DarkGray),
    )
}
