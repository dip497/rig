use std::collections::HashMap;
use std::path::PathBuf;

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

fn collect_names(dir: &PathBuf, names: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && p.join("SKILL.md").exists() {
            names.insert(e.file_name().to_string_lossy().to_string());
        }
    }
}

