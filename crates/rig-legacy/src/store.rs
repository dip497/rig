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

    // ── Agent ─────────────────────────────────────────────────────────────

    #[test]
    fn test_agent_resolved_skill_dir_global() {
        let agent = Agent {
            name: "Test".into(),
            key: 't',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.test/skills"),
            project_skill_dir: None,
            markers: vec![],
        };
        let resolved = agent.resolved_skill_dir(None);
        assert!(!resolved.to_string_lossy().contains('~'));
        assert!(resolved.to_string_lossy().contains(".test/skills"));
    }

    #[test]
    fn test_agent_resolved_skill_dir_project() {
        let agent = Agent {
            name: "Test".into(),
            key: 't',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.test/skills"),
            project_skill_dir: Some(PathBuf::from(".test/skills")),
            markers: vec![],
        };
        let proj = PathBuf::from("/tmp/myproject");
        let resolved = agent.resolved_skill_dir(Some(&proj));
        assert_eq!(resolved, PathBuf::from("/tmp/myproject/.test/skills"));
    }

    #[test]
    fn test_agent_resolved_skill_dir_no_project_skill_dir() {
        let agent = Agent {
            name: "Test".into(),
            key: 't',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.test/skills"),
            project_skill_dir: None,
            markers: vec![],
        };
        let proj = PathBuf::from("/tmp/myproject");
        // Falls back to global skill_dir since project_skill_dir is None
        let resolved = agent.resolved_skill_dir(Some(&proj));
        assert!(resolved.to_string_lossy().contains(".test/skills"));
        assert!(!resolved.to_string_lossy().contains("myproject"));
    }

    #[test]
    fn test_agent_has_signal_in_marker() {
        let tmp = std::env::temp_dir().join(format!("rig-agent-signal-{}", std::process::id()));
        let _ = std::fs::create_dir_all(tmp.join(".claude"));
        let agent = Agent {
            name: "Test".into(),
            key: 't',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.test/skills"),
            project_skill_dir: None,
            markers: vec![".claude".into()],
        };
        assert!(agent.has_signal_in(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_agent_no_signal() {
        let tmp = std::env::temp_dir().join(format!("rig-agent-nosignal-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let agent = Agent {
            name: "Test".into(),
            key: 't',
            color: "White".into(),
            skill_dir: PathBuf::from("~/.test/skills"),
            project_skill_dir: None,
            markers: vec![".claude".into()],
        };
        assert!(!agent.has_signal_in(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_agent_colors() {
        let green = Agent { name: "G".into(), key: 'g', color: "Green".into(), skill_dir: PathBuf::from("~/.g"), project_skill_dir: None, markers: vec![] };
        let cyan = Agent { name: "C".into(), key: 'c', color: "Cyan".into(), skill_dir: PathBuf::from("~/.c"), project_skill_dir: None, markers: vec![] };
        let unknown = Agent { name: "U".into(), key: 'u', color: "Chartreuse".into(), skill_dir: PathBuf::from("~/.u"), project_skill_dir: None, markers: vec![] };
        assert_eq!(green.color(), ratatui::style::Color::Green);
        assert_eq!(cyan.color(), ratatui::style::Color::Cyan);
        assert_eq!(unknown.color(), ratatui::style::Color::White);
    }

    // ── enable/disable with sandbox ──────────────────────────────────────

    struct SkillSandbox {
        store: PathBuf,
        agent_dir: PathBuf,
        project_dir: PathBuf,
        tmp_base: PathBuf,
    }

    impl SkillSandbox {
        fn new() -> Self {
            let pid = std::process::id();
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let tmp_base = std::env::temp_dir().join(format!("rig-store-test-{pid}-{ts}"));
            let _ = std::fs::remove_dir_all(&tmp_base);
            let store = tmp_base.join("store");
            let agent_dir = tmp_base.join("agent-skills");
            let project_dir = tmp_base.join("project");
            std::fs::create_dir_all(&store).unwrap();
            std::fs::create_dir_all(&agent_dir).unwrap();
            std::fs::create_dir_all(&project_dir).unwrap();
            Self { store, agent_dir, project_dir, tmp_base }
        }

        fn add_store_skill(&self, name: &str) {
            let dir = self.store.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
        }

        fn agent(&self) -> Agent {
            Agent {
                name: "TestAgent".into(),
                key: 't',
                color: "White".into(),
                skill_dir: self.agent_dir.clone(),
                project_skill_dir: Some(PathBuf::from(".test/skills")),
                markers: vec![],
            }
        }
    }

    impl Drop for SkillSandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.tmp_base);
        }
    }

    #[test]
    fn test_enable_creates_symlink() {
        let sb = SkillSandbox::new();
        sb.add_store_skill("hello");
        let agent = sb.agent();
        let store_path = sb.store.join("hello");

        // Manually create symlink (bypass enable_skill which uses skill_store())
        let link_dir = &sb.agent_dir;
        let link = link_dir.join("hello");
        std::os::unix::fs::symlink(&store_path, &link).unwrap();

        assert!(link.exists());
        let target = std::fs::read_link(&link).unwrap();
        assert_eq!(target, store_path);
    }

    #[test]
    fn test_symlink_overwrites_existing() {
        let sb = SkillSandbox::new();
        sb.add_store_skill("hello");
        let store_path = sb.store.join("hello");
        let link = sb.agent_dir.join("hello");

        // Create initial symlink
        std::os::unix::fs::symlink(&store_path, &link).unwrap();
        assert!(link.exists());

        // Recreate (should overwrite)
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&store_path, &link).unwrap();
        assert!(link.exists());
    }

    #[test]
    fn test_disable_removes_symlink() {
        let sb = SkillSandbox::new();
        sb.add_store_skill("hello");
        let store_path = sb.store.join("hello");
        let link = sb.agent_dir.join("hello");
        std::os::unix::fs::symlink(&store_path, &link).unwrap();
        assert!(link.exists());

        // Remove
        std::fs::remove_file(&link).unwrap();
        assert!(!link.exists());
        // Store should still exist
        assert!(store_path.exists());
    }

    // ── MCP config I/O ────────────────────────────────────────────────────

    #[test]
    fn test_read_mcp_json_empty() {
        let tmp = std::env::temp_dir().join(format!("rig-mcp-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("empty.json");
        std::fs::write(&path, "{}").unwrap();
        let config = read_mcp_json(&path);
        assert!(config.is_object());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_read_write_mcp_json_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("rig-mcp-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "test-server": {
                    "command": "node",
                    "args": ["server.js"]
                }
            }
        });
        write_mcp_json(&path, &config).unwrap();
        assert!(path.exists());

        let read_back = read_mcp_json(&path);
        let servers = read_back.get("mcpServers").unwrap().as_object().unwrap();
        assert!(servers.contains_key("test-server"));
        assert_eq!(
            servers["test-server"]["command"].as_str().unwrap(),
            "node"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_delete_mcp_from_file() {
        let tmp = std::env::temp_dir().join(format!("rig-mcp-del-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "keep": {"command": "keep-cmd"},
                "delete": {"command": "del-cmd"}
            }
        });
        write_mcp_json(&path, &config).unwrap();

        delete_mcp_from_file("delete", &path).unwrap();

        let after = read_mcp_json(&path);
        let servers = after.get("mcpServers").unwrap().as_object().unwrap();
        assert!(servers.contains_key("keep"));
        assert!(!servers.contains_key("delete"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_delete_mcp_from_file_nonexistent() {
        let tmp = std::env::temp_dir().join(format!("rig-mcp-nodel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("mcp.json");
        write_mcp_json(&path, &serde_json::json!({"mcpServers": {}})).unwrap();

        // Should not error on missing key
        delete_mcp_from_file("absent", &path).unwrap();

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_delete_mcp_from_file_missing_file() {
        // Should not error on missing file
        let result = delete_mcp_from_file("x", &PathBuf::from("/tmp/does-not-exist-rig-test"));
        assert!(result.is_ok());
    }

    // ── is_project_dir ────────────────────────────────────────────────────

    #[test]
    fn test_is_project_dir_rejects_container() {
        let tmp = std::env::temp_dir().join(format!("rig-container-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // Create 3+ subdirs with .git → treated as workspace container
        for i in 0..4 {
            let _ = std::fs::create_dir_all(tmp.join(format!("proj{i}/.git")));
        }
        assert!(!is_project_dir(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_is_project_dir_rejects_home() {
        assert!(!is_project_dir(&home()));
    }

    // ── RigConfig serialization ───────────────────────────────────────────

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut config = RigConfig::default();
        config.projects.push(ProjectEntry {
            name: "test-proj".into(),
            path: PathBuf::from("/tmp/test-proj"),
        });
        config.disabled_mcps.insert("path::server".into());

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: RigConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.projects.len(), 1);
        assert_eq!(parsed.projects[0].name, "test-proj");
        assert!(parsed.disabled_mcps.contains("path::server"));
    }

    #[test]
    fn test_config_default_agents_complete() {
        let config = RigConfig::default();
        let names: Vec<&str> = config.agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"Claude"), "Missing Claude agent");
        assert!(names.contains(&"Cursor"), "Missing Cursor agent");
        assert!(names.contains(&"Codex"), "Missing Codex agent");
        assert!(names.contains(&"Windsurf"), "Missing Windsurf agent");
        assert!(names.contains(&"Cline"), "Missing Cline agent");
        assert!(names.contains(&"Copilot"), "Missing Copilot agent");
        assert!(names.contains(&"Gemini"), "Missing Gemini agent");
        assert!(names.contains(&"Roo"), "Missing Roo agent");
        assert_eq!(config.agents.len(), 8);
    }

    #[test]
    fn test_agents_have_unique_keys() {
        let config = RigConfig::default();
        let keys: std::collections::HashSet<char> = config.agents.iter().map(|a| a.key).collect();
        assert_eq!(keys.len(), config.agents.len(), "Agent keys must be unique");
    }

    #[test]
    fn test_agents_have_skill_dirs() {
        let config = RigConfig::default();
        for agent in &config.agents {
            assert!(!agent.skill_dir.to_string_lossy().is_empty(), "{} has no skill_dir", agent.name);
            assert!(agent.skill_dir.to_string_lossy().contains("/"), "{} skill_dir looks wrong", agent.name);
        }
    }
}
