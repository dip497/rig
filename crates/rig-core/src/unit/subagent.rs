//! Subagent — a scoped assistant definition (Claude Code `.claude/agents/*.md`).
//!
//! Agents that lack a subagent concept (e.g. plain Codex) receive a
//! downgraded install — the adapter decides (rule + command, or skipped
//! with a warning).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub body: String,
}
