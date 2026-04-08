use std::path::PathBuf;
use std::time::Instant;

use ratatui::widgets::ListState;

use crate::mcp::McpEntry;
use crate::skills::Skill;
use crate::store::{self, RigConfig};

// ── Screens ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Skills x Projects matrix — the default and most powerful view
    Matrix,
    /// MCP servers grouped by source
    Mcp,
    /// Focused view of a single project's skills + MCPs
    #[allow(dead_code)]
    ProjectDetail(usize),
    /// Full-screen help overlay
    Help,
}

// ── Modes (within any screen) ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Filter(String),
    Confirm(ConfirmAction),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    BulkEnable(AgentScope),
    BulkDisable(AgentScope),
    DeleteMcp { name: String, source_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentScope {
    All,
    #[allow(dead_code)]
    One(usize),
}

// ── Focus (which panel has keyboard focus) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Sidebar,
    Content,
}

// ── Matrix state (2D cursor for the matrix view) ────────────────────────────

#[derive(Debug, Clone)]
pub struct MatrixState {
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// How many project columns are scrolled off the left
    pub scroll_col: usize,
    /// How many rows are scrolled off the top
    pub scroll_row: usize,
    /// Visible column count (updated by draw)
    pub visible_cols: usize,
    /// Visible row count (updated by draw)
    pub visible_rows: usize,
}

impl Default for MatrixState {
    fn default() -> Self {
        Self {
            cursor_row: 0,
            cursor_col: 0,
            scroll_col: 0,
            scroll_row: 0,
            visible_cols: 0,
            visible_rows: 0,
        }
    }
}

// ── Status bar with auto-expiry ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Status {
    pub msg: String,
    pub kind: StatusKind,
    pub created: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusKind {
    Ok,
    Err,
    Info,
}

impl Status {
    pub fn ok(msg: impl Into<String>) -> Self {
        Self { msg: msg.into(), kind: StatusKind::Ok, created: Instant::now() }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self { msg: msg.into(), kind: StatusKind::Err, created: Instant::now() }
    }

    pub fn info(msg: impl Into<String>) -> Self {
        Self { msg: msg.into(), kind: StatusKind::Info, created: Instant::now() }
    }

    /// Returns true if this status has expired (older than 4 seconds)
    pub fn is_expired(&self) -> bool {
        self.created.elapsed().as_secs() >= 4
    }
}

// ── The App ─────────────────────────────────────────────────────────────────

pub struct App {
    pub config: RigConfig,
    pub screen: Screen,
    pub prev_screen: Option<Screen>,
    pub mode: Mode,
    pub focus: Focus,

    // Data
    pub skills: Vec<Skill>,
    pub mcp_entries: Vec<McpEntry>,

    // Project selection (None = Global)
    pub project_idx: Option<usize>,

    // UI state
    pub matrix: MatrixState,
    pub list_state: ListState,
    pub sidebar_state: ListState,
    pub status: Option<Status>,

    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let config = store::load_config();
        let mut app = Self {
            config,
            screen: Screen::Matrix,
            prev_screen: None,
            mode: Mode::Normal,
            focus: Focus::Content,
            skills: Vec::new(),
            mcp_entries: Vec::new(),
            project_idx: None,
            matrix: MatrixState::default(),
            list_state: ListState::default(),
            sidebar_state: ListState::default(),
            status: None,
            should_quit: false,
        };
        app.sidebar_state.select(Some(0));
        app.rescan();

        let skill_count = app.skills.len();
        let project_count = app.config.projects.len();
        app.status = Some(Status::info(format!(
            "{} skills, {} projects",
            skill_count, project_count,
        )));

        app
    }

    // ── Data ────────────────────────────────────────────────────────────

    pub fn project_dir(&self) -> Option<PathBuf> {
        self.project_idx
            .and_then(|i| self.config.projects.get(i).map(|p| p.path.clone()))
    }

    pub fn rescan(&mut self) {
        let dir = self.project_dir();
        self.skills = crate::skills::scan(&self.config.agents, dir.as_ref());
        self.mcp_entries = crate::mcp::scan(dir.as_ref());
    }

    // ── Filtered views ──────────────────────────────────────────────────

    pub fn filter_query(&self) -> &str {
        match &self.mode {
            Mode::Filter(q) => q.as_str(),
            _ => "",
        }
    }

    pub fn filtered_skills(&self) -> Vec<&Skill> {
        let q = self.filter_query().to_lowercase();
        self.skills
            .iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn filtered_mcp(&self) -> Vec<&McpEntry> {
        let q = self.filter_query().to_lowercase();
        self.mcp_entries
            .iter()
            .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
            .collect()
    }

    // ── Status helpers ──────────────────────────────────────────────────

    pub fn set_status(&mut self, status: Status) {
        self.status = Some(status);
    }

    /// Call this each frame to clear expired status messages
    pub fn tick_status(&mut self) {
        if let Some(ref s) = self.status {
            if s.is_expired() {
                self.status = None;
            }
        }
    }

    // ── Screen navigation ───────────────────────────────────────────────

    pub fn go_to(&mut self, screen: Screen) {
        self.prev_screen = Some(self.screen.clone());
        self.screen = screen;
        self.mode = Mode::Normal;
    }

    pub fn go_back(&mut self) {
        if let Some(prev) = self.prev_screen.take() {
            self.screen = prev;
        }
        self.mode = Mode::Normal;
    }

    // ── Project columns for the matrix view ─────────────────────────────

    /// Returns ("Global", None) followed by (project_name, Some(project_idx))
    pub fn matrix_columns(&self) -> Vec<(String, Option<usize>)> {
        let mut cols = vec![("GLOBAL".into(), None)];
        for (i, proj) in self.config.projects.iter().enumerate() {
            cols.push((proj.name.clone(), Some(i)));
        }
        cols
    }

    /// For a given matrix column, get the project_dir to use for skill lookups
    pub fn column_project_dir(&self, col_project_idx: Option<usize>) -> Option<PathBuf> {
        col_project_idx.and_then(|i| self.config.projects.get(i).map(|p| p.path.clone()))
    }

    // ── Clamp helpers ───────────────────────────────────────────────────

    pub fn clamp_matrix(&mut self) {
        let row_count = self.filtered_skills().len();
        let col_count = self.matrix_columns().len();
        if row_count > 0 {
            self.matrix.cursor_row = self.matrix.cursor_row.min(row_count - 1);
        } else {
            self.matrix.cursor_row = 0;
        }
        if col_count > 0 {
            self.matrix.cursor_col = self.matrix.cursor_col.min(col_count - 1);
        } else {
            self.matrix.cursor_col = 0;
        }
    }
}
