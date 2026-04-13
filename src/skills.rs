use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::store::Agent;

/// A skill — `enabled` maps agent name → is this skill active for that agent
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    /// Exists in ~/.rig/skills/ (central store)
    pub in_store: bool,
    /// Agent name → enabled. Stable across agent reordering.
    pub enabled: HashMap<String, bool>,
}

impl Skill {
    /// Is this skill enabled for the given agent?
    pub fn is_enabled(&self, agent_name: &str) -> bool {
        self.enabled.get(agent_name).copied().unwrap_or(false)
    }

    /// Is this skill enabled for any agent?
    pub fn any_enabled(&self) -> bool {
        self.enabled.values().any(|&v| v)
    }
}

pub fn scan(agents: &[Agent], project_dir: Option<&PathBuf>) -> Vec<Skill> {
    let store = crate::store::skill_store();
    let mut names = std::collections::BTreeSet::new();

    // Collect from store
    collect_names(&store, &mut names);
    // Collect from each agent dir (catches non-managed skills too)
    for agent in agents {
        collect_names(&agent.resolved_skill_dir(project_dir), &mut names);
    }

    let mut skills: Vec<Skill> = names
        .into_iter()
        .map(|name| {
            let store_path = store.join(&name);
            let in_store = store_path.is_dir();

            let enabled: HashMap<String, bool> = agents
                .iter()
                .map(|agent| {
                    let is_on = agent.resolved_skill_dir(project_dir).join(&name).exists();
                    (agent.name.clone(), is_on)
                })
                .collect();

            Skill { name, in_store, enabled }
        })
        .collect();

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Load short descriptions for all skills in the store (for preview bar).
pub fn load_descriptions() -> HashMap<String, String> {
    let store = crate::store::skill_store();
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&store) else { return map };
    for e in entries.flatten() {
        let p = e.path();
        let skill_md = p.join("SKILL.md");
        if skill_md.exists() {
            let fm = parse_frontmatter(&skill_md);
            if let Some(desc) = fm.get("description") {
                let name = e.file_name().to_string_lossy().to_string();
                let truncated = if desc.len() > 120 {
                    format!("{}...", &desc[..117])
                } else {
                    desc.clone()
                };
                map.insert(name, truncated);
            }
        }
    }
    map
}

fn collect_names(dir: &PathBuf, names: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && p.join("SKILL.md").exists() {
            names.insert(e.file_name().to_string_lossy().to_string());
        }
    }
}

// ── Skill detail (parsed SKILL.md) ──────────────────────────────────────────

/// Rich metadata for a single skill, parsed from its SKILL.md.
#[derive(Debug, Clone, Default)]
pub struct SkillDetail {
    pub name: String,
    pub description: String,
    pub version: String,
    pub creator: String,
    pub license: String,
    pub compatibility: String,
    pub effort: String,
    /// Absolute path to the skill directory in the store
    pub store_path: PathBuf,
    /// True if the store entry is a symlink (linked, not copied)
    pub is_symlink: bool,
    /// What the symlink points to (if is_symlink)
    pub symlink_target: Option<PathBuf>,
    /// Number of files in the skill directory
    pub file_count: usize,
    /// Lock entry source (e.g. "github:owner/repo")
    pub source: Option<String>,
    /// Short commit hash from lock file
    pub commit: Option<String>,
}

/// Load rich detail for a skill by name.
/// Returns `None` if the skill doesn't exist in the store.
pub fn load_detail(name: &str) -> Option<SkillDetail> {
    let store_path = crate::store::skill_store().join(name);
    if !store_path.exists() {
        return None;
    }

    let skill_md = store_path.join("SKILL.md");
    let fm = if skill_md.exists() {
        parse_frontmatter(&skill_md)
    } else {
        HashMap::new()
    };

    let is_symlink = store_path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    let symlink_target = if is_symlink {
        std::fs::read_link(&store_path).ok()
    } else {
        None
    };

    let file_count = count_files(&store_path);

    // Check lock file for source/commit
    let lock = crate::lock::read();
    let (source, commit) = if let Some(entry) = lock.skills.get(name) {
        (
            Some(entry.source.clone()),
            if entry.commit.len() >= 7 {
                Some(entry.commit[..7].to_string())
            } else {
                Some(entry.commit.clone())
            },
        )
    } else {
        (None, None)
    };

    Some(SkillDetail {
        name: fm.get("name").cloned().unwrap_or_else(|| name.to_string()),
        description: fm.get("description").cloned().unwrap_or_default(),
        version: fm.get("version").cloned().unwrap_or_else(|| "—".into()),
        creator: fm.get("creator").cloned().unwrap_or_default(),
        license: fm.get("license").cloned().unwrap_or_default(),
        compatibility: fm.get("compatibility").cloned().unwrap_or_default(),
        effort: fm.get("effort").cloned().unwrap_or_default(),
        store_path,
        is_symlink,
        symlink_target,
        file_count,
        source,
        commit,
    })
}

fn parse_frontmatter(path: &Path) -> HashMap<String, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();

    // Find opening ---
    let body = match content.strip_prefix("---") {
        Some(b) => b,
        None => return map,
    };
    // Find closing ---
    let end = match body.find("\n---") {
        Some(e) => e,
        None => return map,
    };
    let fm = &body[..end];

    for line in fm.lines() {
        let line = line.trim();
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim().to_lowercase();
            let val = line[colon + 1..].trim().trim_matches('"').trim_matches('\'').to_string();
            if !key.is_empty() && !val.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn count_files(dir: &Path) -> usize {
    let mut count = 0usize;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" { continue; }
        if p.is_file() {
            count += 1;
        } else if p.is_dir() {
            count += count_files(&p);
        }
    }
    count
}

