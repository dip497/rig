mod app;
mod events;
mod mcp;
mod skills;
mod store;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use std::io;

fn main() -> anyhow::Result<()> {
    // Migrate skills to central store before starting TUI
    let config = store::load_config();
    let _migration = store::migrate_all(&config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn run(terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    let mut app = app::App::new();

    loop {
        terminal.draw(|f| ui::draw(&mut app, f))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            events::handle_event(&mut app, ev);
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
