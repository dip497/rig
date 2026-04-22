//! Install scope: global (`~/.rig/`), project (`./.rig/`), or Claude's
//! per-project `local` override.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Project,
    /// Claude-only per-project override; typically gitignored. Valid
    /// only for MCP units on the Claude adapter (see
    /// `docs/MCP-SUPPORT.md` §8). Any other adapter/unit-type
    /// combination must reject with `AdapterError::Unsupported`.
    Local,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Global => "global",
            Self::Project => "project",
            Self::Local => "local",
        })
    }
}
