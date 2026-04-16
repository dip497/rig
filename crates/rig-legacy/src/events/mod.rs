pub mod matrix;
pub mod mcp;
pub mod project;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Focus, Mode, Screen, Status};
use crate::installer;

/// Top-level event dispatcher. Routes to the right screen handler.
pub fn handle_event(app: &mut App, ev: Event) {
    let Event::Key(key) = ev else { return };

    // ── Skill detail overlay: close on Esc or Enter ───
    if matches!(app.mode, Mode::SkillDetail(_)) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.mode = Mode::Normal;
            }
            _ => {}
        }
        return;
    }

    // ── Install mode: capture URL input ───────────────
    if let Mode::Install(ref mut input) = app.mode {
        match key.code {
            KeyCode::Esc => {
                app.mode = Mode::Normal;
                app.set_status(Status::info("Cancelled"));
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            KeyCode::Enter => {
                let source = input.trim().to_string();
                if source.is_empty() {
                    app.mode = Mode::Normal;
                    return;
                }
                app.mode = Mode::Normal;
                app.set_status(Status::info(format!("Installing {}…", source)));
                let result = installer::tui_install(&source, &app.config);
                if let Some(err) = result.error {
                    app.set_status(Status::err(format!("Install failed: {err}")));
                } else if result.installed.is_empty() {
                    app.set_status(Status::err("No skills found in source"));
                } else {
                    let names = result.installed.join(", ");
                    app.set_status(Status::ok(format!("Installed: {names}")));
                    app.rescan();
                }
            }
            _ => {}
        }
        return;
    }

    // ── Filter mode: capture all keys ──────────────────
    if matches!(app.mode, Mode::Filter(_)) {
        match key.code {
            KeyCode::Esc => {
                // Discard edits — revert filter and exit.
                app.filter.clear();
                app.mode = Mode::Normal;
                app.clamp_matrix();
                clamp_mcp_selection(app);
            }
            KeyCode::Enter => {
                // Commit filter; stay applied after exit.
                app.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                if let Mode::Filter(ref mut q) = app.mode {
                    q.pop();
                }
                app.filter.pop();
                app.clamp_matrix();
                clamp_mcp_selection(app);
            }
            // Navigate underlying list while filter is active
            // Arrow keys always navigate; Ctrl+j/k also navigate (vim-friendly)
            KeyCode::Down => navigate_down(app),
            KeyCode::Up => navigate_up(app),
            KeyCode::Right => {
                if let Screen::Matrix = app.screen {
                    let col_count = app.matrix_columns().len();
                    if col_count > 0 && app.matrix.cursor_col < col_count - 1 {
                        app.matrix.cursor_col += 1;
                        let visible = app.matrix.visible_cols;
                        if visible > 0
                            && app.matrix.cursor_col >= app.matrix.scroll_col + visible
                        {
                            app.matrix.scroll_col = app.matrix.cursor_col - visible + 1;
                        }
                    }
                }
            }
            KeyCode::Left => {
                if let Screen::Matrix = app.screen {
                    if app.matrix.cursor_col > 0 {
                        app.matrix.cursor_col -= 1;
                        if app.matrix.cursor_col < app.matrix.scroll_col {
                            app.matrix.scroll_col = app.matrix.cursor_col;
                        }
                    }
                }
            }
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+j/k/n/p navigate while typing — don't append to filter
                match c {
                    'j' | 'n' => navigate_down(app),
                    'k' | 'p' => navigate_up(app),
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                if let Mode::Filter(ref mut q) = app.mode {
                    q.push(c);
                }
                app.filter.push(c);
                app.clamp_matrix();
                clamp_mcp_selection(app);
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
            app.mode = Mode::Filter(app.filter.clone());
            return;
        }
        KeyCode::Char('\\') => {
            // Clear persistent filter
            if !app.filter.is_empty() {
                app.filter.clear();
                app.clamp_matrix();
                clamp_mcp_selection(app);
                app.set_status(Status::info("Filter cleared"));
            }
            return;
        }
        KeyCode::Char('i') => {
            app.mode = Mode::Install(String::new());
            return;
        }
        KeyCode::Char('r') => {
            app.rescan();
            app.set_status(Status::ok("Rescanned"));
            return;
        }
        KeyCode::Char('S') => {
            app.sort_mode = app.sort_mode.next();
            app.set_status(Status::info(format!("Sort: {}", app.sort_mode.label())));
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

fn navigate_down(app: &mut App) {
    match app.screen {
        Screen::Matrix => {
            let len = app.filtered_skills().len();
            if len > 0 && app.matrix.cursor_row < len - 1 {
                app.matrix.cursor_row += 1;
            }
        }
        Screen::Mcp => {
            let len = app.filtered_mcp().len();
            let cur = app.list_state.selected().unwrap_or(0);
            if cur < len.saturating_sub(1) {
                app.list_state.select(Some(cur + 1));
            }
        }
        _ => {}
    }
}

fn navigate_up(app: &mut App) {
    match app.screen {
        Screen::Matrix => {
            if app.matrix.cursor_row > 0 {
                app.matrix.cursor_row -= 1;
            }
        }
        Screen::Mcp => {
            let cur = app.list_state.selected().unwrap_or(0);
            if cur > 0 {
                app.list_state.select(Some(cur - 1));
            }
        }
        _ => {}
    }
}

fn clamp_mcp_selection(app: &mut App) {
    let len = app.filtered_mcp().len();
    if len == 0 {
        app.list_state.select(Some(0));
        return;
    }
    let cur = app.list_state.selected().unwrap_or(0);
    if cur >= len {
        app.list_state.select(Some(len - 1));
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
        ConfirmAction::UninstallSkill { name } => {
            match store::uninstall_skill(name, &app.config.agents, &app.config.projects) {
                Ok(n) => {
                    app.set_status(Status::ok(format!(
                        "Uninstalled '{}' — removed {} links + store", name, n
                    )));
                    app.rescan();
                    app.clamp_matrix();
                }
                Err(e) => app.set_status(Status::err(format!("Uninstall failed: {}", e))),
            }
        }
    }
}
