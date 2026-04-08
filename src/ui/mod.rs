pub mod matrix;
pub mod mcp;
pub mod project;
pub mod widgets;
pub mod help;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::{App, Screen};

pub fn draw(app: &mut App, f: &mut Frame) {
    app.tick_status();

    // Sidebar takes ~25% of width, min 28, max 40
    let sidebar_width = (f.area().width / 4).clamp(28, 40);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(40)])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Length(1),  // tabs / screen indicator
            Constraint::Min(5),    // content
            Constraint::Length(1),  // search / filter bar
            Constraint::Length(1),  // status
            Constraint::Length(2),  // help
        ])
        .split(chunks[1]);

    widgets::draw_sidebar(app, f, chunks[0]);
    widgets::draw_header(app, f, main[0]);
    widgets::draw_screen_tabs(app, f, main[1]);

    match &app.screen {
        Screen::Matrix => matrix::draw(app, f, main[2]),
        Screen::Mcp => mcp::draw(app, f, main[2]),
        Screen::ProjectDetail(idx) => {
            let idx = *idx;
            project::draw(app, f, main[2], idx);
        }
        Screen::Help => help::draw(app, f, main[2]),
    }

    widgets::draw_filter_bar(app, f, main[3]);
    widgets::draw_status(app, f, main[4]);
    widgets::draw_help_bar(app, f, main[5]);
}
