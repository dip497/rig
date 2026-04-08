use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::app::{App, Focus};

/// Width of the skill name column
const NAME_COL: u16 = 22;
/// Minimum width for a project column (agent dots need at least this)
const MIN_COL_WIDTH: u16 = 8;

pub fn draw(app: &mut App, f: &mut Frame, area: Rect) {
    let filtered_count = app.filtered_skills().len();
    let skills_empty = app.skills.is_empty();

    if filtered_count == 0 {
        let msg = if skills_empty {
            "No skills found. Install with: npx skills add <name>"
        } else {
            "No skills match the current filter"
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let columns = app.matrix_columns();
    let agent_count = app.config.agents.len();

    // Calculate dynamic column widths based on name lengths
    let col_widths: Vec<u16> = columns
        .iter()
        .map(|(name, _)| {
            let name_len = name.len() as u16 + 2; // +2 for padding
            let agent_dots = (agent_count as u16) * 2 + 1; // "C . " per agent
            name_len.max(agent_dots).max(MIN_COL_WIDTH)
        })
        .collect();

    // Figure out how many columns fit
    let visible_cols = {
        let available = area.width.saturating_sub(NAME_COL + 2);
        let mut count = 0;
        let mut used: u16 = 0;
        for w in col_widths.iter().skip(app.matrix.scroll_col) {
            if used + w + 1 > available {
                break;
            }
            used += w + 1; // +1 for column spacing
            count += 1;
        }
        count.max(1)
    };
    let visible_rows = (area.height as usize).saturating_sub(1); // -1 for header

    // Store visible dimensions
    app.matrix.visible_cols = visible_cols;
    app.matrix.visible_rows = visible_rows;

    // Auto-scroll vertically
    if app.matrix.cursor_row >= app.matrix.scroll_row + visible_rows {
        app.matrix.scroll_row = app.matrix.cursor_row - visible_rows + 1;
    }
    if app.matrix.cursor_row < app.matrix.scroll_row {
        app.matrix.scroll_row = app.matrix.cursor_row;
    }

    // Auto-scroll horizontally
    if visible_cols > 0 && app.matrix.cursor_col >= app.matrix.scroll_col + visible_cols {
        app.matrix.scroll_col = app.matrix.cursor_col - visible_cols + 1;
    }
    if app.matrix.cursor_col < app.matrix.scroll_col {
        app.matrix.scroll_col = app.matrix.cursor_col;
    }

    let scroll_col = app.matrix.scroll_col;
    let scroll_row = app.matrix.scroll_row;

    // Re-borrow after mutable section
    let filtered = app.filtered_skills();
    let agents = &app.config.agents;
    let visible_columns: Vec<_> = columns.iter().skip(scroll_col).take(visible_cols).collect();
    let visible_widths: Vec<u16> = col_widths.iter().skip(scroll_col).take(visible_cols).copied().collect();

    // ── Header row ──────────────────────────────────────
    let mut header_cells = vec![Cell::from(Span::styled(
        format!("{:<width$}", "SKILL", width = NAME_COL as usize),
        Style::default().fg(Color::White).bold(),
    ))];

    for (i, (col_name, _)) in visible_columns.iter().enumerate() {
        let w = visible_widths[i] as usize;
        header_cells.push(Cell::from(Span::styled(
            format!("{:<width$}", col_name, width = w),
            Style::default().fg(Color::Cyan).bold(),
        )));
    }

    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::DarkGray))
        .height(1);

    // ── Data rows ───────────────────────────────────────
    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .skip(scroll_row)
        .take(visible_rows)
        .map(|(row_idx, skill)| {
            let is_cursor_row = row_idx == app.matrix.cursor_row;

            let name_style = if skill.in_store {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            let name_suffix = if !skill.in_store { " !" } else { "" };
            let name_cell = Cell::from(Span::styled(
                format!(
                    "{:<width$}{}",
                    &skill.name,
                    name_suffix,
                    width = (NAME_COL as usize).saturating_sub(2)
                ),
                name_style,
            ));

            let mut cells = vec![name_cell];

            for (vis_idx, (_, col_proj_idx)) in visible_columns.iter().enumerate() {
                let actual_col = scroll_col + vis_idx;
                let is_cursor_cell = is_cursor_row && actual_col == app.matrix.cursor_col;

                let mut agent_spans = vec![];
                for agent in agents.iter() {
                    let is_on = if col_proj_idx.is_none() {
                        skill.is_enabled(&agent.name)
                    } else {
                        let proj_dir =
                            col_proj_idx.and_then(|i| app.config.projects.get(i).map(|p| &p.path));
                        if let Some(pd) = proj_dir {
                            agent
                                .resolved_skill_dir(Some(pd))
                                .join(&skill.name)
                                .exists()
                        } else {
                            skill.is_enabled(&agent.name)
                        }
                    };

                    let (symbol, color) = if is_on {
                        (agent.key.to_uppercase().to_string(), agent.color())
                    } else {
                        (".".into(), Color::DarkGray)
                    };

                    let style = if is_cursor_cell {
                        Style::default().fg(color).bold().bg(Color::DarkGray)
                    } else if is_on {
                        Style::default().fg(color).bold()
                    } else {
                        Style::default().fg(color)
                    };

                    agent_spans.push(Span::styled(symbol, style));
                    agent_spans.push(Span::raw(" "));
                }

                cells.push(Cell::from(Line::from(agent_spans)));
            }

            let row_style = if is_cursor_row && app.focus == Focus::Content {
                Style::default().bg(Color::Rgb(30, 30, 40))
            } else {
                Style::default()
            };

            Row::new(cells).style(row_style).height(1)
        })
        .collect();

    // ── Column constraints ──────────────────────────────
    let mut widths: Vec<Constraint> = vec![Constraint::Length(NAME_COL)];
    for w in &visible_widths {
        widths.push(Constraint::Length(*w));
    }

    // ── Scroll indicator ────────────────────────────────
    let total_rows = filtered.len();
    let total_cols = columns.len();
    let mut indicators = Vec::new();

    if total_rows > visible_rows {
        indicators.push(format!("row {}/{}", app.matrix.cursor_row + 1, total_rows));
    }
    if total_cols > visible_cols {
        let showing_end = (scroll_col + visible_cols).min(total_cols);
        indicators.push(format!("col {}-{}/{}", scroll_col + 1, showing_end, total_cols));
    }

    if !indicators.is_empty() {
        let indicator_text = format!(" [{}] ", indicators.join("  "));
        let ind_width = indicator_text.len() as u16;
        f.render_widget(
            Paragraph::new(Span::styled(
                indicator_text,
                Style::default().fg(Color::DarkGray),
            )),
            Rect {
                x: area.x + area.width.saturating_sub(ind_width),
                y: area.y,
                width: ind_width,
                height: 1,
            },
        );
    }

    let table = Table::new(rows, widths).header(header).column_spacing(1);
    f.render_widget(table, area);
}
