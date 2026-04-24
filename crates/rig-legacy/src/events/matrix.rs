use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AgentScope, App, ConfirmAction, Focus, Mode, Status};
use crate::store;

pub fn handle(app: &mut App, key: KeyEvent) {
    let filtered_len = app.filtered_skills().len();
    let col_count = app.matrix_columns().len();

    match key.code {
        // ── Navigation ─────────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            if filtered_len > 0 && app.matrix.cursor_row < filtered_len - 1 {
                app.matrix.cursor_row += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.matrix.cursor_row > 0 {
                app.matrix.cursor_row -= 1;
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if col_count > 0 && app.matrix.cursor_col < col_count - 1 {
                app.matrix.cursor_col += 1;
                // Auto-scroll columns to keep cursor visible
                let visible = app.matrix.visible_cols;
                if visible > 0 && app.matrix.cursor_col >= app.matrix.scroll_col + visible {
                    app.matrix.scroll_col = app.matrix.cursor_col - visible + 1;
                }
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if app.matrix.cursor_col > 0 {
                app.matrix.cursor_col -= 1;
                // Auto-scroll columns to keep cursor visible
                if app.matrix.cursor_col < app.matrix.scroll_col {
                    app.matrix.scroll_col = app.matrix.cursor_col;
                }
            } else {
                // Move focus to sidebar
                app.focus = Focus::Sidebar;
            }
        }
        KeyCode::Char('g') => app.matrix.cursor_row = 0,
        KeyCode::Char('G') => {
            if filtered_len > 0 {
                app.matrix.cursor_row = filtered_len - 1;
            }
        }
        // Scroll project columns
        KeyCode::Char('>') => {
            if col_count > 0 {
                app.matrix.scroll_col = (app.matrix.scroll_col + 1).min(col_count.saturating_sub(1));
            }
        }
        KeyCode::Char('<') => {
            if app.matrix.scroll_col > 0 {
                app.matrix.scroll_col -= 1;
            }
        }

        // ── Toggle (Space) ─────────────────────────────
        // Toggle all agents for the skill at cursor in the current column's project context
        KeyCode::Char(' ') => {
            toggle_at_cursor(app);
        }

        // ── Toggle by agent number (1-9) ───────────────
        KeyCode::Char(c @ '1'..='9') => {
            let agent_idx = (c as usize) - ('1' as usize);
            if agent_idx < app.config.agents.len() {
                toggle_skill_for_agent(app, agent_idx);
            }
        }

        // ── Open project detail for column under cursor ──
        KeyCode::Char('p') => {
            let columns = app.matrix_columns();
            if let Some((_, Some(proj_idx))) = columns.get(app.matrix.cursor_col) {
                app.go_to(crate::app::Screen::ProjectDetail(*proj_idx));
            }
        }

        // ── Skill detail overlay ───────────────────────
        KeyCode::Enter => {
            let filtered = app.filtered_skills();
            if let Some(skill) = filtered.get(app.matrix.cursor_row) {
                app.mode = Mode::SkillDetail(skill.name.clone());
            }
        }

        // ── Uninstall skill ────────────────────────────
        KeyCode::Char('u') => {
            let filtered = app.filtered_skills();
            if let Some(skill) = filtered.get(app.matrix.cursor_row) {
                if skill.in_store {
                    let name = skill.name.clone();
                    app.mode = Mode::Confirm(ConfirmAction::UninstallSkill { name: name.clone() });
                    app.set_status(Status::info(format!("Uninstall '{}'? removes from store + all agents (y/n)", name)));
                } else {
                    app.set_status(Status::err("Skill not in store"));
                }
            }
        }

        // ── Bulk enable/disable ────────────────────────
        KeyCode::Char('E') => {
            app.mode = Mode::Confirm(ConfirmAction::BulkEnable(AgentScope::All));
            app.set_status(Status::info("Enable ALL skills for all agents? y/n"));
        }
        KeyCode::Char('D') => {
            app.mode = Mode::Confirm(ConfirmAction::BulkDisable(AgentScope::All));
            app.set_status(Status::info("Disable ALL skills for all agents? y/n"));
        }

        _ => {}
    }
}

fn toggle_at_cursor(app: &mut App) {
    let filtered = app.filtered_skills();
    let Some(skill) = filtered.get(app.matrix.cursor_row) else { return };

    if !skill.in_store {
        app.set_status(Status::err(format!(
            "'{}' not in store — press [i] to install or run: rig install <source>",
            skill.name
        )));
        return;
    }

    let columns = app.matrix_columns();
    let Some((_, col_proj_idx)) = columns.get(app.matrix.cursor_col) else { return };
    let proj_dir = app.column_project_dir(*col_proj_idx);
    let name = skill.name.clone();

    // Toggle each agent
    let mut toggled = Vec::new();
    for agent in &app.config.agents {
        let agent_dir = agent.resolved_skill_dir(proj_dir.as_ref());
        let is_on = agent_dir.join(&name).exists();

        let result = if is_on {
            store::disable_skill(&name, agent, proj_dir.as_ref())
                .map(|_| format!("-{}", agent.key.to_uppercase()))
        } else {
            store::enable_skill(&name, agent, proj_dir.as_ref())
                .map(|_| format!("+{}", agent.key.to_uppercase()))
        };

        match result {
            Ok(msg) => toggled.push(msg),
            Err(e) => {
                app.set_status(Status::err(format!("{}: {}", agent.name, e)));
                app.rescan();
                return;
            }
        }
    }

    app.set_status(Status::ok(format!("{} {}", name, toggled.join(" "))));
    app.rescan();
}

fn toggle_skill_for_agent(app: &mut App, agent_idx: usize) {
    let filtered = app.filtered_skills();
    let Some(skill) = filtered.get(app.matrix.cursor_row) else { return };

    if !skill.in_store {
        app.set_status(Status::err(format!(
            "'{}' not in store",
            skill.name
        )));
        return;
    }

    // Use the cursor column's project context, not the sidebar selection
    let columns = app.matrix_columns();
    let Some((col_name, col_proj_idx)) = columns.get(app.matrix.cursor_col) else { return };
    let col_name = col_name.clone();
    let proj_dir = app.column_project_dir(*col_proj_idx);

    let agent = &app.config.agents[agent_idx];
    let name = skill.name.clone();
    let agent_name = agent.name.clone();

    // Check actual filesystem state for this project column
    let is_on = agent.resolved_skill_dir(proj_dir.as_ref()).join(&name).exists();

    let result = if is_on {
        store::disable_skill(&name, &app.config.agents[agent_idx], proj_dir.as_ref())
    } else {
        store::enable_skill(&name, &app.config.agents[agent_idx], proj_dir.as_ref()).map(|_| ())
    };

    match result {
        Ok(_) => {
            let action = if is_on { "disabled" } else { "enabled" };
            app.set_status(Status::ok(format!(
                "{} {} for {} [{}]", name, action, agent_name, col_name
            )));
        }
        Err(e) => app.set_status(Status::err(format!("{}", e))),
    }
    app.rescan();
}
