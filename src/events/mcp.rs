use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, ConfirmAction, Focus, Mode, Status};
use crate::store;

pub fn handle(app: &mut App, key: KeyEvent) {
    let filtered_len = app.filtered_mcp().len();

    match key.code {
        // ── Navigation ─────────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            let cur = app.list_state.selected().unwrap_or(0);
            if cur < filtered_len.saturating_sub(1) {
                app.list_state.select(Some(cur + 1));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let cur = app.list_state.selected().unwrap_or(0);
            if cur > 0 {
                app.list_state.select(Some(cur - 1));
            }
        }
        KeyCode::Char('g') => app.list_state.select(Some(0)),
        KeyCode::Char('G') => {
            if filtered_len > 0 {
                app.list_state.select(Some(filtered_len - 1));
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.focus = Focus::Sidebar;
        }

        // ── Soft toggle (x) — non-destructive ─────────
        KeyCode::Char('x') | KeyCode::Char(' ') => {
            toggle_mcp(app);
        }

        // ── Hard delete (d) — with confirmation ────────
        KeyCode::Char('d') => {
            let filtered = app.filtered_mcp();
            let Some(idx) = app.list_state.selected() else { return };
            let Some(entry) = filtered.get(idx) else { return };

            let name = entry.name.clone();
            let path = entry.source.path.clone();
            app.mode = Mode::Confirm(ConfirmAction::DeleteMcp {
                name: name.clone(),
                source_path: path,
            });
            app.set_status(Status::info(format!(
                "Delete '{}' from config file? y/n",
                name
            )));
        }

        _ => {}
    }
}

fn toggle_mcp(app: &mut App) {
    let filtered = app.filtered_mcp();
    let Some(idx) = app.list_state.selected() else { return };
    let Some(entry) = filtered.get(idx) else { return };

    let disable_key = entry.disable_key();
    let name = entry.name.clone();

    match store::toggle_mcp_disabled(&mut app.config, &disable_key) {
        Ok(is_now_disabled) => {
            let action = if is_now_disabled { "disabled" } else { "enabled" };
            app.set_status(Status::ok(format!("{} {}", name, action)));
            // Rescan with updated config to reflect disabled state
            let dir = app.project_dir();
            app.mcp_entries = crate::mcp::scan_with_config(dir.as_ref(), Some(&app.config));
        }
        Err(e) => app.set_status(Status::err(format!("{}", e))),
    }
}
