# Introduction

Rig is a **distribution and management layer for agent coding context**.

It installs, pins, syncs, and shares the things that make an AI coding
assistant useful on your codebase — skills, MCP servers, rules, hooks,
subagents, and plugins — across every agent you use, with per-project and
global scope, and with drift-safe reconciliation so your team stays in sync
without stepping on anyone's local edits.

## The problem

Every agent coding tool has its own home for context:

| Unit | Claude Code | Codex |
|------|-------------|-------|
| Skill | `~/.claude/skills/<name>/SKILL.md` | `~/.codex/skills/<name>/SKILL.md` |
| MCP server | `.mcp.json` (JSON) | `~/.codex/config.toml` (TOML) |
| Rules | `CLAUDE.md` | `AGENTS.md` |
| Hooks | `.claude/settings.json` hooks block | `~/.codex/config.toml` `[[hooks]]` |
| Commands | `.claude/commands/*.md` | `~/.codex/prompts/*.md` |
| Subagents | `.claude/agents/*.md` | — (Claude-only) |
| Plugins | marketplace | — (Claude-only) |

If you use two agents you maintain two copies of the same stack. If a
teammate clones your repo, there is no portable manifest that says "this
project expects these skills, these MCPs, this rule set, these hooks". If a
skill's upstream author ships v2 you cannot tell whether your local copy
has diverged.

## The answer

```toml
# .rig/rig.toml
[project]
name = "my-app"
schema = "rig/v1"

[agents]
targets = ["claude", "codex"]

[bundle."frontend-react"]
skills = ["github:acme/react-review@v1.2"]
mcps   = ["smithery:figma-mcp"]
rules  = ["github:acme/react-ts-rules"]
```

```bash
rig install    # materialises everything across all selected agents
rig sync       # checks drift, warns before overwriting local edits
```

`.rig/rig.toml` + `.rig/rig.lock` are the project's agent-context source of
truth. Commit them. Teammates `rig install`. Everyone is identical.

## Who Rig is for

- **Polyglot devs** running Claude Code and Codex side-by-side.
- **OSS maintainers** who want contributors to get the right agent setup
  in one command when they clone a repo.
- **Startup teams** who need the same stack across 5–20 engineers without
  manual drift.
- **Course creators / influencers** who ship an agent stack with their
  content.

Rig is explicitly **not** for:

- Replacing Anthropic's Claude Code plugin marketplace. We consume it.
- Inventing a new skill or MCP format. SKILL.md and MCP are fine standards.
- Running agents. We manage their context, not their execution.

## What makes Rig different

1. **Cross-agent from day one.** Not "Claude with Cursor later"; genuine
   multi-target translation via adapters.
2. **Drift-safe sync.** Rig tracks install-time, current-disk, and upstream
   SHAs. Never overwrites silently. Five resolution modes.
3. **Pluggable everywhere.** Unit types, agent adapters, source backends,
   frontends (CLI / GUI / future TUI / community), and commands all ship
   as plugins via a stable JSON protocol.
4. **GitOps-first.** `.rig/rig.toml` committed → team sync by reflex.
5. **Respects existing standards.** Consumes Claude plugin marketplace,
   SKILL.md, AGENTS.md, MCPB, Smithery, openskills — doesn't fork them.

Next: [concepts](./concepts.md), [architecture](./architecture.md),
[vision](./vision.md).
