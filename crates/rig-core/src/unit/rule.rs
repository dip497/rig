//! Rule — a markdown instruction file (CLAUDE.md, AGENTS.md, .cursorrules, …).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub body: String,
    /// Hint to adapters about where this rule fits in the agent's
    /// instruction hierarchy (user / project / directory-local).
    #[serde(default)]
    pub placement: RulePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RulePlacement {
    #[default]
    Project,
    User,
    /// Lives next to a specific directory — path given by caller.
    Directory,
}
