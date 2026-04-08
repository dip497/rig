pub mod matrix;
pub mod mcp;
pub mod project;

use crossterm::event::{Event, KeyCode, KeyEvent};

use crate::app::{App, Focus, Mode, Screen, Status};

/// Top-level event dispatcher. Routes to the right screen handler.
pub fn handle_event(app: &mut App, ev: Event) {
    let Event::Key(key) = ev else { return };

    // ── Filter mode: capture all keys ──────────────────
    if let Mode::Filter(ref mut query) = app.mode {
        match key.code {
            KeyCode::Esc => {
                app.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                // Keep the filter active? No — we exit filter mode.
                // The filtered_skills/mcp will show unfiltered again.
                app.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                query.pop();
                if query.is_empty() {
                    app.mode = Mode::Normal;
                }
            }
            KeyCode::Char(c) => {
                query.push(c);
            }
            _ => {}
        }
        return;
    }

    // ── Confirm mode: y/n ──────────────────────────────
    if let Mode::Confirm(ref action) = app.mode {
        let action = action.clone();
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                execute_confirm(app, &action);
                app.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.set_status(Status::info("Cancelled"));
                app.mode = Mode::Normal;
            }
            _ => {}
        }
        return;
    }

    // ── Global keys (work on any screen) ───────────────
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            if app.screen == Screen::Help {
                app.go_back();
            } else {
                app.go_to(Screen::Help);
            }
            return;
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Filter(String::new());
            return;
        }
        KeyCode::Char('r') => {
            app.rescan();
            app.set_status(Status::ok("Rescanned"));
            return;
        }
        _ => {}
    }

    // ── Screen switching ───────────────────────────────
    if app.focus != Focus::Sidebar {
        match key.code {
            KeyCode::Char('s') if !matches!(app.screen, Screen::Matrix) => {
                app.screen = Screen::Matrix;
                app.focus = Focus::Content;
                app.clamp_matrix();
                return;
            }
            KeyCode::Char('m') if !matches!(app.screen, Screen::Mcp) => {
                app.screen = Screen::Mcp;
                app.focus = Focus::Content;
                app.list_state.select(Some(0));
                return;
            }
            _ => {}
        }
    }

    // ── Focus switching ────────────────────────────────
    match key.code {
        KeyCode::Esc => {
            match app.screen {
                Screen::Help => app.go_back(),
                Screen::ProjectDetail(_) => app.go_back(),
                _ => {
                    if app.focus == Focus::Sidebar {
                        app.focus = Focus::Content;
                    } else {
                        app.should_quit = true;
                    }
                }
            }
            return;
        }
        _ => {}
    }

    // ── Sidebar navigation ─────────────────────────────
    if app.focus == Focus::Sidebar {
        handle_sidebar(app, key);
        return;
    }

    // ── Screen-specific handling ────────────────────────
    match &app.screen {
        Screen::Matrix => matrix::handle(app, key),
        Screen::Mcp => mcp::handle(app, key),
        Screen::ProjectDetail(idx) => {
            let idx = *idx;
            project::handle(app, key, idx);
        }
        Screen::Help => {
            // Help screen: Esc/? already handled above
        }
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    let max = app.config.projects.len(); // 0=Global, 1..=max = projects

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let cur = app.sidebar_state.selected().unwrap_or(0);
            if cur < max {
                app.sidebar_state.select(Some(cur + 1));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let cur = app.sidebar_state.selected().unwrap_or(0);
            if cur > 0 {
                app.sidebar_state.select(Some(cur - 1));
            }
        }
        KeyCode::Char('g') => app.sidebar_state.select(Some(0)),
        KeyCode::Char('G') => app.sidebar_state.select(Some(max)),
        KeyCode::Char('l') | KeyCode::Right => {
            app.focus = Focus::Content;
        }
        KeyCode::Enter => {
            let idx = app.sidebar_state.selected().unwrap_or(0);
            match &app.screen {
                Screen::Matrix => {
                    // In matrix: jump cursor to that project's column
                    // idx 0 = Global (column 0), idx N = project N-1 (column N)
                    app.matrix.cursor_col = idx;
                    let name = if idx == 0 {
                        "GLOBAL".to_string()
                    } else {
                        app.config.projects[idx - 1].name.clone()
                    };
                    app.set_status(Status::info(format!("Jumped to {}", name)));
                    app.focus = Focus::Content;
                }
                Screen::Mcp => {
                    // In MCP view: filter MCPs to that project
                    if idx == 0 {
                        app.project_idx = None;
                        app.set_status(Status::info("Global MCPs"));
                    } else {
                        let proj_idx = idx - 1;
                        app.project_idx = Some(proj_idx);
                        let name = app.config.projects[proj_idx].name.clone();
                        app.set_status(Status::info(format!("MCPs for {}", name)));
                    }
                    app.rescan();
                    app.list_state.select(Some(0));
                    app.focus = Focus::Content;
                }
                _ => {
                    app.focus = Focus::Content;
                }
            }
        }
        KeyCode::Char('p') => {
            // Open project detail for the selected sidebar item
            let idx = app.sidebar_state.selected().unwrap_or(0);
            if idx > 0 {
                let proj_idx = idx - 1;
                app.go_to(Screen::ProjectDetail(proj_idx));
                app.focus = Focus::Content;
            }
        }
        _ => {}
    }
}

fn execute_confirm(app: &mut App, action: &crate::app::ConfirmAction) {
    use crate::app::{AgentScope, ConfirmAction};
    use crate::store;

    match action {
        ConfirmAction::BulkEnable(scope) => {
            let dir = app.project_dir();
            let (mut total, mut errors) = (0, Vec::new());
            let agents: Vec<_> = match scope {
                AgentScope::All => app.config.agents.clone(),
                AgentScope::One(i) => vec![app.config.agents[*i].clone()],
            };
            for agent in &agents {
                match store::enable_all(agent, dir.as_ref()) {
                    Ok(n) => total += n,
                    Err(e) => errors.push(format!("{}: {}", agent.name, e)),
                }
            }
            if errors.is_empty() {
                app.set_status(Status::ok(format!("Enabled {} skill links", total)));
            } else {
                app.set_status(Status::err(errors.join(", ")));
            }
            app.rescan();
        }
        ConfirmAction::BulkDisable(scope) => {
            let dir = app.project_dir();
            let (mut total, mut errors) = (0, Vec::new());
            let agents: Vec<_> = match scope {
                AgentScope::All => app.config.agents.clone(),
                AgentScope::One(i) => vec![app.config.agents[*i].clone()],
            };
            for agent in &agents {
                match store::disable_all(agent, dir.as_ref()) {
                    Ok(n) => total += n,
                    Err(e) => errors.push(format!("{}: {}", agent.name, e)),
                }
            }
            if errors.is_empty() {
                app.set_status(Status::ok(format!("Disabled {} skill links", total)));
            } else {
                app.set_status(Status::err(errors.join(", ")));
            }
            app.rescan();
        }
        ConfirmAction::DeleteMcp { name, source_path } => {
            match store::delete_mcp_from_file(name, source_path) {
                Ok(_) => {
                    app.set_status(Status::ok(format!("Deleted {} from config", name)));
                    app.rescan();
                }
                Err(e) => app.set_status(Status::err(format!("{}", e))),
            }
        }
    }
}
