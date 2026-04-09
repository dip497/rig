mod app;
mod events;
mod mcp;
mod migrate;
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
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("migrate") => return migrate::run(),
        Some("project") => return handle_project(&args[2..]),
        Some("--help" | "-h") => {
            print_help();
            return Ok(());
        }
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            eprintln!("Run `rig --help` for usage.");
            std::process::exit(1);
        }
        None => {}
    }

    // Launch TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let res = run_tui(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn print_help() {
    println!("rig — manage AI skills & MCP servers\n");
    println!("Usage:");
    println!("  rig                        Launch the TUI");
    println!("  rig migrate                Migrate skills into ~/.rig/skills/");
    println!("  rig project add <path>     Add a project to manage");
    println!("  rig project remove <name>  Remove a project");
    println!("  rig project list           List managed projects");
    println!("  rig --help                 Show this help");
}

fn handle_project(args: &[String]) -> anyhow::Result<()> {
    match args.first().map(|s| s.as_str()) {
        Some("add") => {
            let path_str = args.get(1).ok_or_else(|| {
                anyhow::anyhow!("Usage: rig project add <path>")
            })?;

            let path = PathBuf::from(path_str);
            let path = if path.is_relative() {
                std::env::current_dir()?.join(&path)
            } else {
                path
            };
            let path = path.canonicalize().map_err(|_| {
                anyhow::anyhow!("Path does not exist: {}", path.display())
            })?;

            if !path.is_dir() {
                anyhow::bail!("Not a directory: {}", path.display());
            }

            let mut config = store::load_config();

            // Check if already added
            if config.projects.iter().any(|p| p.path == path) {
                println!("Already managed: {}", path.display());
                return Ok(());
            }

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            config.projects.push(store::ProjectEntry {
                name: name.clone(),
                path: path.clone(),
            });
            store::save_config(&config)?;

            println!("Added project: {} ({})", name, path.display());
            println!("Run `rig migrate` to import any existing skills.");
        }
        Some("remove") => {
            let name = args.get(1).ok_or_else(|| {
                anyhow::anyhow!("Usage: rig project remove <name>")
            })?;

            let mut config = store::load_config();
            let before = config.projects.len();
            config.projects.retain(|p| p.name != *name);

            if config.projects.len() == before {
                anyhow::bail!("Project not found: {name}");
            }

            store::save_config(&config)?;
            println!("Removed project: {name}");
        }
        Some("list") => {
            let config = store::load_config();
            if config.projects.is_empty() {
                println!("No managed projects. Add one with: rig project add <path>");
                return Ok(());
            }
            println!("Managed projects:\n");
            for project in &config.projects {
                let exists = project.path.is_dir();
                let marker = if exists { " " } else { "!" };
                println!("  {} {} ({})", marker, project.name, project.path.display());
            }
        }
        _ => {
            println!("Usage:");
            println!("  rig project add <path>     Add a project to manage");
            println!("  rig project remove <name>  Remove a project");
            println!("  rig project list           List managed projects");
        }
    }
    Ok(())
}

fn run_tui(terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
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
