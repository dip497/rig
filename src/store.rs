use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Agent definition (config-driven) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    pub name: String,
    pub key: char,
    #[serde(default = "default_color")]
    pub color: String,
    pub skill_dir: PathBuf,
    #[serde(default)]
    pub project_skill_dir: Option<PathBuf>,
    /// Marker dirs this agent creates in projects (e.g. ".claude", ".gsd")
    #[serde(default)]
    pub markers: Vec<String>,
}

fn default_color() -> String {
    "White".into()
}

impl Agent {
    pub fn resolved_skill_dir(&self, project_dir: Option<&PathBuf>) -> PathBuf {
        if let Some(proj) = project_dir {
            if let Some(rel) = &self.project_skill_dir {
                return proj.join(rel);
            }
        }
        shellexpand::tilde(&self.skill_dir.to_string_lossy())
            .to_string()
            .into()
    }

    pub fn color(&self) -> ratatui::style::Color {
        match self.color.to_lowercase().as_str() {
            "green" => ratatui::style::Color::Green,
            "cyan" => ratatui::style::Color::Cyan,
            "magenta" => ratatui::style::Color::Magenta,
            "yellow" => ratatui::style::Color::Yellow,
            "red" => ratatui::style::Color::Red,
            "blue" => ratatui::style::Color::Blue,
            _ => ratatui::style::Color::White,
        }
    }

    /// Check if this agent has a presence in a project directory
    pub fn has_signal_in(&self, project_path: &Path) -> bool {
        for marker in &self.markers {
            if project_path.join(marker).exists() {
                return true;
            }
        }
        if let Some(rel) = &self.project_skill_dir {
            if project_path.join(rel).exists() {
                return true;
            }
        }
        false
    }
}

// ── Config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RigConfig {
    #[serde(default)]
    pub agents: Vec<Agent>,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
    pub last_project: Option<String>,
    /// MCP servers soft-disabled by the user (keyed by "source_path::server_name")
    #[serde(default)]
    pub disabled_mcps: HashSet<String>,
    /// Directories to scan for projects (e.g. ["~/projects", "~/work"])
    #[serde(default)]
    pub search_roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectEntry {
    pub name: String,
    pub path: PathBuf,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            agents: default_agents(),
            projects: Vec::new(),
            last_project: None,
            disabled_mcps: HashSet::new(),
            search_roots: Vec::new(),
        }
    }
}

impl RigConfig {
    /// Merge auto-discovered projects (from search_roots + Claude sessions), persist if new
    pub fn merge_discovered(&mut self) {
        let discovered = discover_projects(&self.search_roots);
        let existing: HashSet<PathBuf> = self.projects.iter().map(|p| p.path.clone()).collect();
        let mut added = false;
        for proj in discovered {
            if !existing.contains(&proj.path) {
                self.projects.push(proj);
                added = true;
            }
        }
        if added {
            let _ = save_config(self);
        }
    }
}

fn default_agents() -> Vec<Agent> {
    vec![
        Agent {
            name: "Claude".into(),
            key: 'c',
            color: "Green".into(),
            skill_dir: PathBuf::from("~/.claude/skills"),
            project_skill_dir: Some(PathBuf::from(".claude/skills")),
            markers: vec![".claude".into()],
        },
        Agent {
            name: "Cursor".into(),
            key: 'u',
            color: "Cyan".into(),
            skill_dir: PathBuf::from("~/.cursor/skills"),
            project_skill_dir: Some(PathBuf::from(".cursor/skills")),
            markers: vec![".cursor".into()],
        },
        Agent {
            name: "Windsurf".into(),
            key: 'w',
            color: "Blue".into(),
            skill_dir: PathBuf::from("~/.codeium/windsurf/skills"),
            project_skill_dir: Some(PathBuf::from(".windsurf/skills")),
            markers: vec![".windsurf".into()],
        },
        Agent {
            name: "Codex".into(),
            key: 'x',
            color: "Yellow".into(),
            skill_dir: PathBuf::from("~/.codex/skills"),
            project_skill_dir: Some(PathBuf::from(".codex/skills")),
            markers: vec![".codex".into()],
        },
        Agent {
            name: "Cline".into(),
            key: 'l',
            color: "Red".into(),
            skill_dir: PathBuf::from("~/.cline/skills"),
            project_skill_dir: Some(PathBuf::from(".cline/skills")),
            markers: vec![".cline".into()],
        },
        Agent {
            name: "Copilot".into(),
            key: 'p',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.copilot/skills"),
            project_skill_dir: Some(PathBuf::from(".copilot/skills")),
            markers: vec![".copilot".into(), ".github".into()],
        },
        Agent {
            name: "Gemini".into(),
            key: 'e',
            color: "Cyan".into(),
            skill_dir: PathBuf::from("~/.gemini/skills"),
            project_skill_dir: Some(PathBuf::from(".gemini/skills")),
            markers: vec![".gemini".into()],
        },
        Agent {
            name: "Roo".into(),
            key: 'r',
            color: "Magenta".into(),
            skill_dir: PathBuf::from("~/.roo/skills"),
            project_skill_dir: Some(PathBuf::from(".roo/skills")),
            markers: vec![".roo".into()],
        },
    ]
}

// ── Paths ───────────────────────────────────────────────────────────────

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

pub fn skill_store() -> PathBuf {
    home().join(".rig/skills")
}

fn config_path() -> PathBuf {
    home().join(".rig/config.json")
}

// ── Config I/O ──────────────────────────────────────────────────────────

pub fn load_config() -> RigConfig {
    let path = config_path();
    let mut config: RigConfig = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        RigConfig::default()
    };
    config.merge_discovered();
    config
}

pub fn save_config(config: &RigConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

// ── Auto-discovery ──────────────────────────────────────────────────────
//
// Claude Code path encoding (confirmed from GitHub issues #9221, #35162,
// #19972, #30244):
//
//   Replace /, \, -, _, spaces, and all non-ASCII chars with '-'
//   Keep only ASCII alphanumeric chars and '-'
//   Leading - represents root /
//
// This is LOSSY — /a/b-c and /a/b/c map to the same encoded name.
// Decoding strategy: use filesystem existence checks to resolve ambiguity
// (credited to @athola's workaround in issue #19972).

/// Decode a Claude-encoded project directory name to a real filesystem path.
///
/// Strategy: split on '-', anchor to known home dir, then greedily try
/// joining consecutive parts (with '-') to find real directory names.
/// At each step, verify against the filesystem.
fn decode_claude_project(encoded: &str) -> Option<PathBuf> {
    let stripped = encoded.strip_prefix('-')?;
    let parts: Vec<&str> = stripped.split('-').collect();
    let home = home();
    let home_str = home.to_str()?;

    // Match home dir prefix first (handles dashes in username like dipendra-sharma)
    let home_segments: Vec<&str> = home_str.trim_start_matches('/').split('/').collect();
    let mut consumed = 0;
    let mut resolved_segments: Vec<String> = Vec::new();
    let mut matched_home = false;

    for seg in &home_segments {
        let seg_sub_parts: Vec<&str> = seg.split('-').collect();
        let needed = seg_sub_parts.len();
        if consumed + needed > parts.len() {
            break;
        }
        if &parts[consumed..consumed + needed] == seg_sub_parts.as_slice() {
            resolved_segments.push(seg.to_string());
            consumed += needed;
            matched_home = true;
        } else {
            break;
        }
    }

    if !matched_home {
        return None;
    }

    // Now resolve remaining parts greedily: try longest join first
    let remaining = &parts[consumed..];
    if remaining.is_empty() {
        let path = PathBuf::from(format!("/{}", resolved_segments.join("/")));
        return if path.is_dir() { Some(path) } else { None };
    }

    // Greedy resolution: at each position, try joining 1..N consecutive parts
    // with '-' and check if that directory exists on disk
    let mut base = PathBuf::from(format!("/{}", resolved_segments.join("/")));
    let mut pos = 0;

    while pos < remaining.len() {
        let mut found = false;
        // Try joining more parts first (greedy — prefer longer real names)
        for width in (1..=remaining.len() - pos).rev() {
            let candidate_name = remaining[pos..pos + width].join("-");
            let candidate_path = base.join(&candidate_name);
            if candidate_path.is_dir() {
                base = candidate_path;
                pos += width;
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
    }

    Some(base)
}

/// Discover projects from Claude's ~/.claude/projects/ session tracking
fn discover_from_claude_sessions() -> Vec<PathBuf> {
    let claude_dir = home().join(".claude/projects");
    if !claude_dir.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&claude_dir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|e| {
            let encoded = e.file_name().to_string_lossy().to_string();
            decode_claude_project(&encoded)
        })
        .filter(|p| is_project_dir(p))
        .collect()
}

/// Discover projects from Claude's active session CWDs
fn discover_from_claude_active() -> Vec<PathBuf> {
    let sessions_dir = home().join(".claude/sessions");
    if !sessions_dir.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "json" {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            let data: serde_json::Value = serde_json::from_str(&content).ok()?;
            let cwd = data.get("cwd")?.as_str()?.to_string();
            let cwd_path = PathBuf::from(&cwd);
            if cwd_path.is_dir() {
                Some(cwd_path)
            } else {
                None
            }
        })
        .filter(|p| is_project_dir(p))
        .collect()
}

/// Agent marker dirs to scan for
const MARKER_DIRS: &[&str] = &[
    ".claude", ".cursor", ".windsurf", ".codex", ".cline",
    ".copilot", ".github", ".gemini", ".roo", ".gsd",
];

/// Parent directories to scan for projects.
/// Only scans dirs explicitly listed in config.search_roots.
fn search_roots(roots: &[String]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|s| PathBuf::from(shellexpand::tilde(s).to_string()))
        .collect()
}

/// Scan parent dirs for subdirectories containing agent marker signals.
/// Scans two levels deep to catch nested structures.
fn discover_from_markers(custom_roots: &[String]) -> Vec<PathBuf> {
    let mut found = Vec::new();

    for root in search_roots(custom_roots) {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };

        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.starts_with("wt-") || name == "worktree" {
                continue;
            }

            if has_any_marker(&dir) {
                found.push(dir.clone());
            }

            // One level deeper (catches ~/Development/project/motadata-itsm-server etc.)
            let Ok(deep_entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for deep_entry in deep_entries.flatten() {
                let deep_dir = deep_entry.path();
                if deep_dir.is_dir() && has_any_marker(&deep_dir) {
                    found.push(deep_dir);
                }
            }
        }
    }

    found
}

fn has_any_marker(dir: &Path) -> bool {
    MARKER_DIRS.iter().any(|m| dir.join(m).is_dir())
}

/// Check if a directory looks like an actual project (not a container dir)
fn is_project_dir(path: &Path) -> bool {
    if !path.is_dir() || path == home().as_path() {
        return false;
    }

    const PROJECT_FILES: &[&str] = &[
        ".git",
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "requirements.txt",
        "pyproject.toml",
        "Gemfile",
        "Makefile",
        "CMakeLists.txt",
        "deno.json",
        "composer.json",
        "CLAUDE.md",
        ".mcp.json",
    ];

    for file in PROJECT_FILES {
        if path.join(file).exists() {
            return true;
        }
    }

    // Reject container dirs: if ≥3 subdirs have .git, this is a workspace root
    let mut project_subdirs = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().join(".git").exists() {
                project_subdirs += 1;
                if project_subdirs >= 3 {
                    return false;
                }
            }
        }
    }

    false
}

/// Full discovery: merge all sources, dedup, sort
pub fn discover_projects(custom_roots: &[String]) -> Vec<ProjectEntry> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut projects: Vec<ProjectEntry> = Vec::new();

    let add = |path: PathBuf, seen: &mut HashSet<PathBuf>, projects: &mut Vec<ProjectEntry>| {
        if seen.insert(path.clone()) {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            projects.push(ProjectEntry { name, path });
        }
    };

    // Source 1: Marker scanning (primary)
    for path in discover_from_markers(custom_roots) {
        if is_project_dir(&path) {
            add(path, &mut seen, &mut projects);
        }
    }

    // Source 2: Claude session history
    for path in discover_from_claude_sessions() {
        if is_project_dir(&path) {
            add(path, &mut seen, &mut projects);
        }
    }

    // Source 3: Claude active CWDs
    for path in discover_from_claude_active() {
        if is_project_dir(&path) {
            add(path, &mut seen, &mut projects);
        }
    }

    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    projects
}

// ── Migration ───────────────────────────────────────────────────────────
// All migration logic lives in src/migrate.rs and is invoked via `rig migrate`.

// ── Skill operations ────────────────────────────────────────────────────

pub fn enable_skill(
    name: &str,
    agent: &Agent,
    project_dir: Option<&PathBuf>,
) -> Result<PathBuf> {
    let store_path = skill_store().join(name);
    anyhow::ensure!(store_path.exists(), "Skill '{}' not in store", name);
    let link_dir = agent.resolved_skill_dir(project_dir);
    std::fs::create_dir_all(&link_dir)?;
    let link_path = link_dir.join(name);
    if link_path.symlink_metadata().is_ok() {
        std::fs::remove_file(&link_path)
            .or_else(|_| std::fs::remove_dir_all(&link_path))?;
    }
    std::os::unix::fs::symlink(&store_path, &link_path)?;
    Ok(link_path)
}

pub fn disable_skill(
    name: &str,
    agent: &Agent,
    project_dir: Option<&PathBuf>,
) -> Result<()> {
    let link_path = agent.resolved_skill_dir(project_dir).join(name);
    let meta = link_path.symlink_metadata();

    if let Ok(m) = meta {
        if m.file_type().is_symlink() {
            // Symlink to store — just remove the link
            std::fs::remove_file(&link_path)?;
        } else if m.is_dir() {
            // Real directory — check if it's a copy of a store skill
            let store_path = skill_store().join(name);
            if store_path.exists() {
                // Store has it, safe to remove the local copy
                std::fs::remove_dir_all(&link_path)?;
            } else {
                anyhow::bail!(
                    "'{}' is a local directory (not from store) — remove manually",
                    link_path.display()
                );
            }
        }
    }
    // If path doesn't exist, already disabled — that's fine
    Ok(())
}

pub fn disable_all(agent: &Agent, project_dir: Option<&PathBuf>) -> Result<usize> {
    let link_dir = agent.resolved_skill_dir(project_dir);
    if !link_dir.is_dir() {
        return Ok(0);
    }
    let store = skill_store();
    let mut count = 0;
    for entry in std::fs::read_dir(&link_dir)? {
        let path = entry?.path();
        if path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            if std::fs::read_link(&path)
                .map(|t| t.starts_with(&store))
                .unwrap_or(false)
            {
                std::fs::remove_file(&path)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

pub fn enable_all(agent: &Agent, project_dir: Option<&PathBuf>) -> Result<usize> {
    let store = skill_store();
    if !store.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&store)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().join("SKILL.md").exists()
            && enable_skill(&name, agent, project_dir).is_ok()
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Remove a skill entirely: all symlinks (global + per-project) + store entry + lock
pub fn uninstall_skill(name: &str, agents: &[Agent], projects: &[ProjectEntry]) -> Result<usize> {
    let mut removed = 0usize;

    // Remove from each agent's global skill dir
    for agent in agents {
        let link = agent.resolved_skill_dir(None).join(name);
        if link.symlink_metadata().is_ok() {
            std::fs::remove_file(&link)
                .or_else(|_| std::fs::remove_dir_all(&link))?;
            removed += 1;
        }
    }

    // Remove from each agent's per-project skill dir
    for proj in projects {
        for agent in agents {
            let link = agent.resolved_skill_dir(Some(&proj.path)).join(name);
            if link.symlink_metadata().is_ok() {
                std::fs::remove_file(&link)
                    .or_else(|_| std::fs::remove_dir_all(&link))?;
                removed += 1;
            }
        }
    }

    // Remove from store
    let store_path = skill_store().join(name);
    if store_path.symlink_metadata().is_ok() {
        if store_path.symlink_metadata()?.file_type().is_symlink() {
            std::fs::remove_file(&store_path)?;
        } else {
            std::fs::remove_dir_all(&store_path)?;
        }
    }

    // Remove from lock file
    let _ = crate::lock::remove(name);

    Ok(removed)
}

// ── MCP operations ──────────────────────────────────────────────────────

/// Soft-toggle: add/remove from disabled_mcps set in config (non-destructive)
pub fn toggle_mcp_disabled(config: &mut RigConfig, disable_key: &str) -> Result<bool> {
    let is_now_disabled = if config.disabled_mcps.contains(disable_key) {
        config.disabled_mcps.remove(disable_key);
        false
    } else {
        config.disabled_mcps.insert(disable_key.to_string());
        true
    };
    save_config(config)?;
    Ok(is_now_disabled)
}

/// Hard delete an MCP entry from its source JSON file (use sparingly)
pub fn delete_mcp_from_file(name: &str, source_path: &Path) -> Result<()> {
    if !source_path.exists() {
        return Ok(());
    }
    let mut config = read_mcp_json(source_path);
    if let Some(obj) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        if obj.remove(name).is_some() {
            write_mcp_json(source_path, &config)?;
        }
    }
    Ok(())
}

fn read_mcp_json(path: &Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(serde_json::json!({}))
}

fn write_mcp_json(path: &Path, config: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_decode_rejects_nonexistent() {
        assert!(decode_claude_project("-nonexistent-path-that-does-not-exist").is_none());
    }

    #[test]
    fn test_claude_decode_rejects_empty() {
        assert!(decode_claude_project("").is_none());
        assert!(decode_claude_project("no-leading-dash").is_none());
    }

    #[test]
    fn test_is_project_dir_needs_marker() {
        // A temp dir with no project markers should not be a project
        let tmp = std::env::temp_dir().join("rig-test-empty");
        let _ = std::fs::create_dir_all(&tmp);
        assert!(!is_project_dir(&tmp));
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn test_is_project_dir_with_git() {
        let tmp = std::env::temp_dir().join("rig-test-git");
        let _ = std::fs::create_dir_all(tmp.join(".git"));
        assert!(is_project_dir(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_default_config_has_agents() {
        let config = RigConfig::default();
        assert!(config.agents.len() >= 2);
        assert!(config.agents.iter().any(|a| a.name == "Claude"));
    }

    #[test]
    fn test_search_roots_empty_by_default() {
        let config = RigConfig::default();
        assert!(config.search_roots.is_empty());
        assert!(search_roots(&config.search_roots).is_empty());
    }

    #[test]
    fn test_search_roots_expands_tilde() {
        let roots = vec!["~/projects".to_string()];
        let expanded = search_roots(&roots);
        assert_eq!(expanded.len(), 1);
        assert!(!expanded[0].to_string_lossy().contains('~'));
    }
}
