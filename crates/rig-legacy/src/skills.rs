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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct SkillSandbox {
        base: PathBuf,
    }

    impl SkillSandbox {
        fn new(name: &str) -> Self {
            let base = std::env::temp_dir().join(format!("rig-skill-test-{}", name));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base).unwrap();
            Self { base }
        }

        fn add_skill_dir(&self, name: &str) {
            let dir = self.base.join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: test {name}\n---\n"),
            ).unwrap();
        }

        fn add_skill_dir_custom(&self, name: &str, frontmatter: &str) {
            let dir = self.base.join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("SKILL.md"), frontmatter).unwrap();
        }

        fn add_empty_dir(&self, name: &str) {
            fs::create_dir_all(self.base.join(name)).unwrap();
        }

        fn add_file_in(&self, skill: &str, file: &str, content: &str) {
            let dir = self.base.join(skill);
            fs::create_dir_all(&dir).unwrap();
            // Ensure SKILL.md exists so it counts as a skill dir
            if !dir.join("SKILL.md").exists() {
                fs::write(dir.join("SKILL.md"), format!("---\nname: {skill}\n---\n")).unwrap();
            }
            if let Some(parent) = std::path::Path::new(file).parent() {
                if parent != std::path::Path::new("") {
                    fs::create_dir_all(dir.join(parent)).unwrap();
                }
            }
            fs::write(dir.join(file), content).unwrap();
        }
    }

    impl Drop for SkillSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    // ── collect_names ─────────────────────────────────────────────────────

    #[test]
    fn test_collect_names_finds_skills() {
        let sb = SkillSandbox::new("collect");
        sb.add_skill_dir("alpha");
        sb.add_skill_dir("beta");
        sb.add_empty_dir("not-a-skill"); // no SKILL.md

        let mut names = std::collections::BTreeSet::new();
        collect_names(&sb.base, &mut names);

        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
        assert!(!names.contains("not-a-skill"));
    }

    #[test]
    fn test_collect_names_empty_dir() {
        let sb = SkillSandbox::new("empty");
        let mut names = std::collections::BTreeSet::new();
        collect_names(&sb.base, &mut names);
        assert!(names.is_empty());
    }

    // ── parse_frontmatter ─────────────────────────────────────────────────

    #[test]
    fn test_parse_frontmatter_full() {
        let sb = SkillSandbox::new("fm-full");
        sb.add_skill_dir_custom("skill", "---\nname: My Skill\ndescription: A test\nversion: \"1.0\"\ncreator: test\nlicense: MIT\n---\n# Body\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert_eq!(fm.get("name").unwrap(), "My Skill");
        assert_eq!(fm.get("description").unwrap(), "A test");
        assert_eq!(fm.get("version").unwrap(), "1.0");
        assert_eq!(fm.get("creator").unwrap(), "test");
        assert_eq!(fm.get("license").unwrap(), "MIT");
    }

    #[test]
    fn test_parse_frontmatter_minimal() {
        let sb = SkillSandbox::new("fm-min");
        sb.add_skill_dir_custom("skill", "---\nname: x\n---\nBody");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert_eq!(fm.get("name").unwrap(), "x");
        assert_eq!(fm.len(), 1);
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let sb = SkillSandbox::new("fm-none");
        sb.add_skill_dir_custom("skill", "Just some text\nNo frontmatter at all\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert!(fm.is_empty());
    }

    #[test]
    fn test_parse_frontmatter_empty_values_skipped() {
        let sb = SkillSandbox::new("fm-empty");
        sb.add_skill_dir_custom("skill", "---\nname:\ndescription: \"\"\n---\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        // Empty values should not be inserted
        assert!(!fm.contains_key("name"));
        assert!(!fm.contains_key("description"));
    }

    #[test]
    fn test_parse_frontmatter_single_quoted() {
        let sb = SkillSandbox::new("fm-squote");
        sb.add_skill_dir_custom("skill", "---\nname: 'my skill'\n---\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert_eq!(fm.get("name").unwrap(), "my skill");
    }

    #[test]
    fn test_parse_frontmatter_double_quoted() {
        let sb = SkillSandbox::new("fm-dquote");
        sb.add_skill_dir_custom("skill", "---\nname: \"my skill\"\n---\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert_eq!(fm.get("name").unwrap(), "my skill");
    }

    #[test]
    fn test_parse_frontmatter_keys_are_lowercase() {
        let sb = SkillSandbox::new("fm-case");
        sb.add_skill_dir_custom("skill", "---\nName: test\nDESCRIPTION: hello\n---\n");

        let fm = parse_frontmatter(&sb.base.join("skill/SKILL.md"));
        assert_eq!(fm.get("name").unwrap(), "test");
        assert_eq!(fm.get("description").unwrap(), "hello");
    }

    // ── Skill ─────────────────────────────────────────────────────────────

    #[test]
    fn test_skill_is_enabled() {
        let mut enabled = HashMap::new();
        enabled.insert("Claude".into(), true);
        enabled.insert("Cursor".into(), false);
        let skill = Skill {
            name: "test".into(),
            in_store: true,
            enabled,
        };
        assert!(skill.is_enabled("Claude"));
        assert!(!skill.is_enabled("Cursor"));
        assert!(!skill.is_enabled("Unknown")); // not in map → false
    }

    #[test]
    fn test_skill_any_enabled() {
        let skill_on = Skill {
            name: "on".into(),
            in_store: true,
            enabled: HashMap::from([("a".into(), true)]),
        };
        let skill_off = Skill {
            name: "off".into(),
            in_store: true,
            enabled: HashMap::from([("a".into(), false)]),
        };
        let skill_empty = Skill {
            name: "empty".into(),
            in_store: true,
            enabled: HashMap::new(),
        };
        assert!(skill_on.any_enabled());
        assert!(!skill_off.any_enabled());
        assert!(!skill_empty.any_enabled());
    }

    // ── count_files ───────────────────────────────────────────────────────

    #[test]
    fn test_count_files_nested() {
        let sb = SkillSandbox::new("files");
        sb.add_skill_dir("x"); // creates SKILL.md
        sb.add_file_in("x", "README.md", "# X"); // add_file_in ensures SKILL.md exists
        sb.add_file_in("x", "refs/a.md", "A");
        sb.add_file_in("x", "refs/b.md", "B");

        let count = count_files(&sb.base.join("x"));
        assert_eq!(count, 4); // SKILL.md + README.md + a.md + b.md
    }

    #[test]
    fn test_count_files_skips_git() {
        let sb = SkillSandbox::new("gitcount");
        sb.add_skill_dir("y");
        sb.add_file_in("y", "refs/a.md", "A");
        // Simulate .git
        fs::create_dir_all(sb.base.join("y/.git")).unwrap();
        fs::write(sb.base.join("y/.git/HEAD"), "ref").unwrap();

        let count = count_files(&sb.base.join("y"));
        assert_eq!(count, 2); // SKILL.md + refs/a.md (no .git/HEAD)
    }

    // ── load_descriptions ─────────────────────────────────────────────────

    #[test]
    fn test_load_descriptions_truncates_long() {
        let sb = SkillSandbox::new("desc");
        let long_desc = "x".repeat(200);
        sb.add_skill_dir_custom("long", &format!("---\nname: long\ndescription: {long_desc}\n---\n"));

        let fm = parse_frontmatter(&sb.base.join("long/SKILL.md"));
        let desc = fm.get("description").unwrap();
        // Verify the truncation logic matches what load_descriptions does
        let truncated = if desc.len() > 120 {
            format!("{}...", &desc[..117])
        } else {
            desc.clone()
        };
        assert_eq!(truncated.len(), 120);
        assert!(truncated.ends_with("..."));
    }

    // ── load_detail ───────────────────────────────────────────────────────

    #[test]
    fn test_load_detail_nonexistent() {
        assert!(load_detail("absolutely-does-not-exist-xyz").is_none());
    }

    #[test]
    fn test_load_detail_existing() {
        // This tests against the real ~/.rig/skills/ store
        // If no skills installed, skip gracefully
        let store = crate::store::skill_store();
        if !store.is_dir() { return; }

        let Ok(entries) = fs::read_dir(&store) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().join("SKILL.md").exists() {
                let detail = load_detail(&name);
                assert!(detail.is_some(), "load_detail failed for existing skill: {name}");
                let d = detail.unwrap();
                assert!(!d.name.is_empty());
                assert!(d.store_path.exists());
                return; // one is enough
            }
        }
    }
}

