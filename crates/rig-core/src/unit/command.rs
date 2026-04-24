//! Command — a slash command body (Claude Code `.claude/commands/*.md`,
//! Codex equivalents, etc.).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Markdown body of the command (prompt template).
    pub body: String,
    /// Allowed tool list, if the host supports per-command tool scoping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
}
