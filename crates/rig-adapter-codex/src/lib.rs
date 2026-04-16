//! `rig-adapter-codex` — OpenAI Codex adapter.
//!
//! Translates canonical Rig unit types into Codex's native formats:
//! - Skills → `~/.codex/skills/<name>/SKILL.md` or `<project>/.codex/skills/`
//! - MCPs → `~/.codex/config.toml` `[mcp_servers.*]` (TOML)
//! - Rules → `AGENTS.md`
//! - Hooks → `~/.codex/config.toml` `[[hooks]]` (narrower event set than Claude)
//! - Commands → `~/.codex/prompts/*.md`
//! - Subagents → **downgrade**: emit a Codex skill with delegation prompt +
//!   warn user that Claude-specific delegation semantics are lost
//! - Plugins → **explode**: extract the plugin's skills/MCPs/rules and install
//!   them individually (Codex has no plugin concept)
