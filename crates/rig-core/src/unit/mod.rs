//! Canonical, agent-neutral unit types.
//!
//! Each variant is the portable representation. Adapters translate to
//! and from their agent-native format via `Converter<A>`.

pub mod command;
pub mod hook;
pub mod mcp;
pub mod plugin;
pub mod rule;
pub mod skill;
pub mod subagent;

pub use command::Command;
pub use hook::{Hook, HookEvent};
pub use mcp::{Mcp, Transport};
pub use plugin::Plugin;
pub use rule::Rule;
pub use skill::Skill;
pub use subagent::Subagent;

use serde::{Deserialize, Serialize};

/// Canonical unit envelope. One of the seven M1 types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Unit {
    Skill(Skill),
    Mcp(Mcp),
    Rule(Rule),
    Hook(Hook),
    Command(Command),
    Subagent(Subagent),
    Plugin(Plugin),
}

/// Discriminator used by manifest / adapter capability declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitType {
    Skill,
    Mcp,
    Rule,
    Hook,
    Command,
    Subagent,
    Plugin,
}

impl Unit {
    #[must_use]
    pub fn unit_type(&self) -> UnitType {
        match self {
            Self::Skill(_) => UnitType::Skill,
            Self::Mcp(_) => UnitType::Mcp,
            Self::Rule(_) => UnitType::Rule,
            Self::Hook(_) => UnitType::Hook,
            Self::Command(_) => UnitType::Command,
            Self::Subagent(_) => UnitType::Subagent,
            Self::Plugin(_) => UnitType::Plugin,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Skill(u) => &u.name,
            Self::Mcp(u) => &u.name,
            Self::Rule(u) => &u.name,
            Self::Hook(u) => &u.name,
            Self::Command(u) => &u.name,
            Self::Subagent(u) => &u.name,
            Self::Plugin(u) => &u.name,
        }
    }
}
