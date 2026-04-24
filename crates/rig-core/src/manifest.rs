//! `rig.toml` — the human-authored project/global manifest.
//!
//! Pure parser. No filesystem access (the caller reads the bytes).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agent::AgentId;
use crate::scope::Scope;
use crate::source::{Source, SourceParseError};

pub const SCHEMA: &str = "rig/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub scope: ScopeConfig,
    /// One entry per named bundle.
    #[serde(default, rename = "bundle")]
    pub bundles: BTreeMap<String, Bundle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default)]
    pub targets: Vec<AgentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ScopeConfig {
    #[serde(default)]
    pub default: Option<Scope>,
}

/// One named bundle. Each field is a list of source strings
/// (e.g. `github:acme/foo@v1`); composition via `bundles`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Bundle {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcps: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("unsupported manifest schema `{0}` (expected `{SCHEMA}`)")]
    BadSchema(String),
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid source in bundle `{bundle}`: {source}")]
    Source {
        bundle: String,
        #[source]
        source: SourceParseError,
    },
}

impl Manifest {
    /// Parse + validate a `rig.toml` from a string.
    ///
    /// # Errors
    /// Returns [`ManifestError`] when TOML is malformed, the schema
    /// tag is not `rig/v1`, or any source string fails to parse.
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        let m: Manifest = toml::from_str(s)?;
        if m.schema != SCHEMA {
            return Err(ManifestError::BadSchema(m.schema));
        }
        // Fail fast on source parsing so later resolver stages trust
        // every string.
        for (name, bundle) in &m.bundles {
            for src in bundle.all_sources() {
                Source::parse(src).map_err(|source| ManifestError::Source {
                    bundle: name.clone(),
                    source,
                })?;
            }
        }
        Ok(m)
    }
}

impl Bundle {
    /// Flat iterator over every source string in the bundle (excluding
    /// the `bundles = [...]` references, which name other bundles by
    /// key, not sources).
    pub fn all_sources(&self) -> impl Iterator<Item = &str> {
        self.skills
            .iter()
            .chain(&self.mcps)
            .chain(&self.rules)
            .chain(&self.hooks)
            .chain(&self.commands)
            .chain(&self.subagents)
            .chain(&self.plugins)
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema = "rig/v1"

[project]
name = "my-app"

[agents]
targets = ["test-agent", "test-agent-2"]

[scope]
default = "project"

[bundle.frontend]
skills = ["github:acme/react-review@v1.2"]
mcps = ["marketplace:figma-mcp"]
rules = ["github:acme/react-ts-rules"]
"#;

    #[test]
    fn parses_sample() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.schema, SCHEMA);
        assert_eq!(m.project.unwrap().name, "my-app");
        assert_eq!(m.agents.targets.len(), 2);
        assert_eq!(m.scope.default, Some(Scope::Project));
        let fe = m.bundles.get("frontend").unwrap();
        assert_eq!(fe.skills.len(), 1);
        assert_eq!(fe.mcps.len(), 1);
    }

    #[test]
    fn rejects_wrong_schema() {
        let bad = r#"schema = "rig/v99""#;
        assert!(matches!(
            Manifest::parse(bad),
            Err(ManifestError::BadSchema(_))
        ));
    }

    #[test]
    fn rejects_bad_source() {
        let bad = r#"
schema = "rig/v1"
[bundle.x]
skills = ["github:nosllash"]
"#;
        assert!(matches!(
            Manifest::parse(bad),
            Err(ManifestError::Source { .. })
        ));
    }
}
