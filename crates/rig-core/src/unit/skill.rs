//! Skill — a `SKILL.md` + optional bundled resources.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Raw frontmatter fields beyond `name` / `description`, preserved
    /// verbatim so agent-specific extensions round-trip.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_frontmatter: BTreeMap<String, toml::Value>,
    /// Markdown body after the frontmatter block.
    pub body: String,
    /// Paths bundled alongside `SKILL.md`, relative to the skill root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    /// Path relative to the skill root (e.g. `references/foo.md`).
    pub path: String,
    pub bytes: Vec<u8>,
}
