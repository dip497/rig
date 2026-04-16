<div align="center">

# Rig

**The distribution & management layer for agent coding context.**

One tool to install, pin, sync, and share skills, MCP servers, rules, hooks,
subagents, and plugins across every agent coding tool — Claude Code, Codex,
and more — with per-project and global scope.

[Docs](./docs/introduction.md) · [Vision](./docs/vision.md) · [Concepts](./docs/concepts.md) · [Architecture](./docs/architecture.md) · [Roadmap](./docs/roadmap.md) · [Decisions](./docs/DECISIONS.md) · [Session warmup](./docs/SESSION-START.md)

</div>

---

> ⚠️ **Rig is under active rewrite.** The pre-0.2 TUI-only codebase has been
> archived to [`crates/rig-legacy/`](./crates/rig-legacy/) as reference. The new
> cross-agent manager is being built in the workspace alongside it. Public
> repo, quiet launch — watch or star to follow along.

## What Rig does (once M1 ships)

```bash
rig init            # detect stack, suggest bundles, pick target agents
rig add skill react-review --source github:acme/skills
rig install         # materialise everything from .rig/rig.toml
rig sync            # drift-safe reconcile across Claude Code + Codex
rig status          # what's installed, drifted, orphaned
```

- **Portable.** One `rig.toml` → same stack on Claude Code and Codex.
- **Drift-safe.** Rig never overwrites your local edits silently.
- **GitOps-friendly.** Commit `.rig/rig.toml` and `.rig/rig.lock`, team syncs automatically.
- **Pluggable.** New agent, new unit type, new source, new UI — all ship as
  plugins. Core never needs to change.

## Why Rig exists

Agent coding tools are exploding. Each one has its own home for skills
(`~/.claude/skills/`, `~/.codex/skills/`), MCPs (`.mcp.json` vs TOML),
rules (`CLAUDE.md` vs `AGENTS.md`), hooks, subagents, plugins. Teams juggle
two or three agents. Nothing today pins an entire stack, sync it across
tools, detects drift, or lets a teammate clone your repo and pick up your
agent setup in one command.

Rig is that layer. It sits **above** every agent's native mechanism (we
consume Anthropic's plugin marketplace, Codex's config, community registries
like Smithery) and **below** your team's shared config in git.

Read more in [docs/introduction.md](./docs/introduction.md).

## Status

- Current focus: workspace scaffolding and public-facing docs.
- Next: `rig-core` unit taxonomy, manifest schema, Claude + Codex adapters.
- Tracking: [docs/roadmap.md](./docs/roadmap.md).

## License

Dual-licensed under **MIT OR Apache-2.0** (core crates). See
[LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE), and
[docs/terms.md](./docs/terms.md).

## Contributing

We're in early design. Issues are open for feedback on architecture and
unit taxonomy. See [docs/contributing.md](./docs/contributing.md) before
opening a PR.

## Acknowledgements

Rig builds on standards others have pioneered — [SKILL.md / Agent Skills](https://agentskills.io),
[AGENTS.md](https://agents.md), [MCP](https://modelcontextprotocol.io),
[MCPB](https://blog.modelcontextprotocol.io/posts/2025-11-20-adopting-mcpb/),
and Anthropic's Claude Code plugin system. See
[docs/comparison.md](./docs/comparison.md) for how Rig composes with each.
