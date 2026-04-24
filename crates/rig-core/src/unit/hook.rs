//! Hook — a shell command wired to an agent lifecycle event.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    pub event: HookEvent,
    /// Optional tool-name or glob filter. Interpretation is adapter-specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    Stop,
    SubagentStop,
    Notification,
    /// Escape hatch for agent-specific events not in the canonical set.
    Other(String),
}
