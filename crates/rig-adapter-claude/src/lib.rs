//! `rig-adapter-claude` — Claude Code adapter.
//!
//! Translates canonical Rig unit types into Claude Code's native formats:
//! - Skills → `<scope>/skills/<name>/SKILL.md`
//! - MCPs → `.mcp.json` blocks
//! - Rules → `CLAUDE.md`
//! - Hooks → `settings.json` `hooks` block
//! - Commands → `.claude/commands/*.md`
//! - Subagents → `.claude/agents/*.md`
//! - Plugins → delegate to `/plugin install` via CLI bridge
