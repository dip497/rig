use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, Screen, Status};

pub fn handle(app: &mut App, key: KeyEvent, project_idx: usize) {
    match key.code {
        // ── Navigate between projects ──────────────────
        KeyCode::Char('n') => {
            let max = app.config.projects.len();
            if project_idx + 1 < max {
                app.screen = Screen::ProjectDetail(project_idx + 1);
                let name = app.config.projects[project_idx + 1].name.clone();
                app.set_status(Status::info(name));
            }
        }
        KeyCode::Char('b') => {
            if project_idx > 0 {
                app.screen = Screen::ProjectDetail(project_idx - 1);
                let name = app.config.projects[project_idx - 1].name.clone();
                app.set_status(Status::info(name));
            }
        }

        // ── Toggle skill by agent number ───────────────
        KeyCode::Char(c @ '1'..='9') => {
            let agent_idx = (c as usize) - ('1' as usize);
            if agent_idx < app.config.agents.len() {
                // TODO: implement project-specific skill toggle
                // Needs a selected skill index within the project view
                app.set_status(Status::info(format!(
                    "Toggle for agent {} — coming soon",
                    app.config.agents[agent_idx].name
                )));
            }
        }

        _ => {}
    }
}
