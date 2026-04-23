//! `rig.lock` — machine-generated resolved install state.
//!
//! One entry per `(unit, agent, scope)` triple. Commit this file to
//! reproduce installs bit-exact across machines.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::AgentId;
use crate::scope::Scope;
use crate::source::{Sha256, Source};
use crate::unit::UnitType;

pub const SCHEMA: &str = "rig/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub schema: String,
    #[serde(default, rename = "lock")]
    pub entries: Vec<LockEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    /// Stable identifier `"<unit_type>/<display_source>"`.
    pub id: String,
    pub unit_type: UnitType,
    pub source: Source,
    /// SHA of the upstream bytes at install time.
    pub source_sha: Sha256,
    /// SHA of the canonical unit bytes Rig wrote.
    pub install_sha: Sha256,
    pub agent: AgentId,
    pub scope: Scope,
    /// Absolute path (or `~`-prefixed) of the primary file written.
    pub path: PathBuf,

    /// Agent-native entry name, for unit types installed as entries
    /// inside an agent-managed config (MCP). `None` for file-backed
    /// units (Skill, Rule, ...). See `docs/MCP-SUPPORT.md` §9.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_name: Option<String>,

    /// Round-tripped metadata the native CLI drops (e.g. MCP
    /// `description`). NOT SHA-significant. Keyed by convention
    /// (`description`, etc.).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LockfileError {
    #[error("unsupported lockfile schema `{0}` (expected `{SCHEMA}`)")]
    BadSchema(String),
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("failed to serialise lockfile: {0}")]
    Serialise(#[from] toml::ser::Error),
}

impl Lockfile {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            entries: Vec::new(),
        }
    }

    /// Parse a `rig.lock` from a string.
    ///
    /// # Errors
    /// Returns [`LockfileError`] on malformed TOML or a non-`rig/v1` schema.
    pub fn parse(s: &str) -> Result<Self, LockfileError> {
        let l: Lockfile = toml::from_str(s)?;
        if l.schema != SCHEMA {
            return Err(LockfileError::BadSchema(l.schema));
        }
        Ok(l)
    }

    /// Serialise to canonical TOML.
    ///
    /// # Errors
    /// Returns [`LockfileError::Serialise`] if TOML encoding fails
    /// (should only happen on invariant-breaking data).
    pub fn to_toml(&self) -> Result<String, LockfileError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_empty() {
        let l = Lockfile::new();
        let s = l.to_toml().unwrap();
        let back = Lockfile::parse(&s).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn roundtrips_with_entry() {
        let mut l = Lockfile::new();
        l.entries.push(LockEntry {
            id: "skill/github:acme/foo".into(),
            unit_type: UnitType::Skill,
            source: Source::Github {
                repo: "acme/foo".into(),
                git_ref: Some("v1".into()),
                path: None,
            },
            source_sha: Sha256::of(b"u"),
            install_sha: Sha256::of(b"i"),
            agent: AgentId::from("test-agent"),
            scope: Scope::Project,
            path: PathBuf::from("~/.claude/skills/foo/SKILL.md"),
            native_name: None,
            extra: BTreeMap::new(),
        });
        let s = l.to_toml().unwrap();
        let back = Lockfile::parse(&s).unwrap();
        assert_eq!(l, back);
    }

    /// Forward-compat: an old lockfile without `native_name` /
    /// `extra` must parse (both fields default via `#[serde(default)]`).
    #[test]
    fn parses_old_lockfile_without_new_fields() {
        let old = r#"schema = "rig/v1"

[[lock]]
id = "skill/github:acme/foo"
unit_type = "skill"
source_sha = "aaa"
install_sha = "bbb"
agent = "test-agent"
scope = "project"
path = "~/.claude/skills/foo/SKILL.md"

[lock.source]
kind = "github"
repo = "acme/foo"
ref = "v1"
"#;
        let parsed = Lockfile::parse(old).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert!(parsed.entries[0].native_name.is_none());
        assert!(parsed.entries[0].extra.is_empty());
    }

    #[test]
    fn rejects_bad_schema() {
        let bad = r#"schema = "rig/v99""#;
        assert!(matches!(
            Lockfile::parse(bad),
            Err(LockfileError::BadSchema(_))
        ));
    }
}
