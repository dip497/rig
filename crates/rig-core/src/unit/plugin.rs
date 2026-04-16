//! Plugin — Claude Code plugin bundle (or analogous host-level plugin
//! on other agents). Canonical form carries the raw manifest plus the
//! bundled files; the adapter unpacks them into the agent's layout.

use serde::{Deserialize, Serialize};

use crate::unit::skill::Resource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Raw plugin manifest (`plugin.json` / equivalent), preserved as a
    /// JSON value so host-specific fields round-trip without loss.
    pub manifest: serde_json::Value,
    /// All files shipped inside the plugin folder, paths relative to
    /// the plugin root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<Resource>,
}
