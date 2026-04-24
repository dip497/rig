# Comparison with related projects

Rig interoperates with more projects than it competes with. This document
is an honest map of the landscape.

## Standards we consume

| Project | What it defines | Rig posture |
|---------|-----------------|-------------|
| [SKILL.md / Agent Skills](https://agentskills.io) | Portable skill file format adopted by Claude, Codex, Cursor, Copilot, Windsurf, Goose, Amp, Mistral, Zed, Roo Code, and 16+ more platforms. | **Consumes.** Rig's `Skill` unit wraps the SKILL.md standard. Translation between agents is file-placement, not format change. |
| [AGENTS.md](https://agents.md) | Project-level rules file donated to the Linux Foundation's AAIF. 60k+ repositories. Backed by OpenAI, Google, Cursor, Amp, Factory. | **Consumes.** Rig's `Rule` unit targets AGENTS.md for Codex and tool-native files (e.g. CLAUDE.md) for others. |
| [MCP](https://modelcontextprotocol.io) | Wire protocol for tools. | **Consumes.** Rig manages MCP server *configs*; MCP itself handles runtime. |
| [MCPB](https://blog.modelcontextprotocol.io/posts/2025-11-20-adopting-mcpb/) | `.mcpb` bundle file (zip + manifest) for MCP server distribution. | **Consumes.** An MCP source backend can unpack MCPB bundles directly. |

## Cross-agent tools Rig composes above

| Project | What it ships | Overlap with Rig |
|---------|----------------|------------------|
| [openskills](https://github.com/numman-ali/openskills) | `npx openskills install` for SKILL.md across Claude / Cursor / Windsurf / Aider / Codex. No lockfile, no team sync, no drift, no other unit types. | **Complementary.** Rig generalises openskills' pattern to every unit type and adds lockfile + drift + composition. An openskills-style source backend is a natural Rig plugin. |
| [Smithery](https://smithery.ai) | 6,000+ MCP server catalog with a cross-vendor CLI installer. MCP only. | **Complementary.** `smithery:` is a first-class Rig source ref. Smithery remains the place to discover MCP servers; Rig handles pinning, drift, team sync. |
| [Composio](https://github.com/ComposioHQ/composio) | 1,000+ toolkit catalog. | **Complementary.** Same role as Smithery for Composio toolkits. |
| [agentopology](https://github.com/agentopology/agentopology) | Declarative `.at` DSL that compiles to seven coding-agent config layouts (Claude, Cursor, Codex, Gemini, Copilot, Kiro, OpenClaw). Stateless. | **Nearest neighbour.** Rig adopts the compile-to-many-targets pattern. Rig adds state (lockfile, drift detection), package-manager semantics (install from source, resolve bundles, conflict handling), and a plugin-based extensibility model. We'd love to reuse work; we use `rig.toml` rather than `.at` for familiarity with the Rust / Python / Node toolchains' conventions. |
| [Claude Code plugin marketplace](https://code.claude.com/docs/en/plugins.md) | Anthropic's plugin system: dependencies, SHA pinning, managed settings, user / project / local / managed scopes, MCPs / skills / hooks / LSPs / monitors bundled per plugin. Claude-only. | **Wrap, don't compete.** `claude-marketplace:` is a first-class Rig source ref. `rig-adapter-claude` delegates to `/plugin install` when a unit came from there. For non-Claude targets, Rig "explodes" a plugin into its components. |

## Tools from the authoring layer

| Project | Focus | Rig relationship |
|---------|-------|------------------|
| [agentcompanies](https://github.com/agentcompanies/agentcompanies) | Markdown spec (COMPANY.md / TEAM.md / AGENTS.md / PROJECT.md / TASK.md / SKILL.md) for declaring multi-agent organisations. Runtime: Paperclip. | **Different layer.** agentcompanies is authoring; Rig is distribution. Rig will ship an `AgentCompany` unit type in M2 that can install a company's agents into compatible hosts. Steal `metadata.sources[]` for lockfile provenance. |
| [Paperclip](https://github.com/paperclipai/paperclip) | Reference runtime for agentcompanies. | Out of scope for Rig. |
| [gsd2-config](https://github.com/jeremymcs/gsd2-config) | Tauri desktop GUI for `.gsd/preferences.md`. Single-agent (GSD only). | **UX inspiration** for `rig-gui` (⌘K palette, dirty tracking, atomic writes, secret redaction, scope toggle). Different scope (GSD-only vs cross-agent). |

## Registry competitors

- **Continue Hub** — single-vendor (Continue.dev only). Rig targets
  cross-agent.
- **skills.sh** (Vercel) — catalog / directory, no installer. Rig uses
  it as a discovery source.
- Nothing currently offers bundle-of-bundles composition across agents
  with a lockfile and team sync. That's the slot Rig claims.

## When you should NOT use Rig

- You use only Claude Code and you are happy with its plugin marketplace.
  Use Anthropic's `/plugin install` directly. Rig is probably overkill.
- You want a runtime for multi-agent orgs. Look at Paperclip.
- You need an agent runtime, not context management. Rig does not run
  anything.
- You want a one-off skill installer for a single laptop. `openskills`
  is lighter.

## When Rig earns its keep

- Two or more coding agents in daily use.
- A team or OSS repo where contributors must share an agent setup.
- A stack that has grown beyond what you can remember to re-create by
  hand.
- Any scenario where "what skills / rules / MCPs should this repo have?"
  is a question worth answering in version control.
