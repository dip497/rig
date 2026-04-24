# Concepts

Rig uses a small, stable vocabulary. If two people agree on these words
they can read the rest of the docs without surprises.

## Unit

The smallest installable thing. Seven built-in kinds:

- **Skill** — a SKILL.md with body + frontmatter. Universal (Claude, Codex,
  many more via the Agent Skills standard).
- **MCP server** — an MCP-protocol tool integration. Universal concept;
  each agent stores the config differently (JSON vs TOML vs YAML).
- **Rule** — instruction text loaded by the agent at session start
  (`CLAUDE.md`, `AGENTS.md`, `.cursor/rules/*.mdc`).
- **Hook** — an event handler that fires on tool use, session start, etc.
  Claude and Codex only; lossy translation elsewhere.
- **Command** — a named prompt invoked as `/command`. Present on Claude
  (`.claude/commands/*.md`) and Codex (`~/.codex/prompts/*.md`).
- **Subagent** — a specialised agent role. Claude-native; Codex has no
  user-facing equivalent so Rig downgrades subagents to skills with a
  delegation prompt plus a warning that delegation semantics are lost.
- **Plugin** — Anthropic's Claude Code plugin (bundles many of the above).
  Claude-only on install; Rig "explodes" a plugin into its components when
  targeting Codex.

Community plugins can register additional unit types (for example
`typescript-config`, `git-hooks`) via the plugin protocol.

## Bundle

A named composition of units — across any unit types, from any sources.
Bundles are how real stacks are shipped and shared.

```toml
[bundle."frontend-react"]
skills = ["github:acme/react-review@v1.2"]
mcps   = ["smithery:figma-mcp", "smithery:linear"]
rules  = ["github:acme/react-ts-rules"]
```

Bundles can depend on other bundles (bundle-of-bundles) with SemVer ranges.
See [architecture.md](./architecture.md#bundle-resolution).

## Stack

Everything the project actually installs — the flattened, resolved set of
units after bundles are unrolled. What `rig.lock` records.

## Source

Where a unit comes from. Rig ships support for:

- `github:owner/repo[@ref]`
- `github:owner/repo/subpath[@ref]`
- Git URL
- `npm:package[@range]`
- `claude-marketplace:plugin@version`
- `local:./relative/path`

New source types ship as plugins via the [plugin protocol](./architecture.md#plugins).

## Agent

A coding agent host Rig installs context into. M1 supports **Claude Code**
and **Codex**. Adapters for new agents are new crates (official) or external
plugin binaries (community). Each adapter declares which unit types it
supports and how it serialises them.

## Scope

Where a unit lives:

- **Global** — `~/.rig/` applied to every project unless overridden.
- **Project** — `./.rig/` applied to this repository only.

`rig init` detects whether it is inside a git repo and defaults to
**project** scope if so, **global** otherwise. The user is always asked to
confirm.

## Drift

The difference between three SHAs Rig tracks per unit:

- **install-time SHA** — what Rig wrote to disk on install.
- **current-disk SHA** — what is on disk right now.
- **upstream-source SHA** — what the source currently serves.

Drift states: `Clean`, `LocalDrift`, `UpstreamDrift`, `BothDrift`, `Orphan`,
`Missing`. Resolved via one of five modes on `rig sync`: `keep`,
`overwrite`, `diff-per-file`, `snapshot-then-overwrite`, `cancel`. Rig
never overwrites silently.

## Adapter

A crate or external binary that knows how to convert a canonical unit into
an agent's native format (and back). Exactly one adapter per agent. The
`Adapter` trait is documented in [architecture.md](./architecture.md#adapters).

## Converter

The per-unit-type, per-agent translation function
(`Converter<Claude> for Skill`, `Converter<Codex> for McpServer`, etc.).
Every new unit type means one Converter impl per adapter.

## Frontend

Any program a human uses to drive Rig: the official `rig` CLI, the `rig-gui`
Tauri app, a community VSCode extension, whatever. Frontends talk to
`rig-core` (linked) or `rig-daemon` (via `rig-api`). Multiple frontends can
run simultaneously when the daemon is available.

## Plugin

An external binary that extends Rig without being compiled into core.
Plugins implement adapters, new unit types, new source backends, or new
frontend surfaces. They communicate via a stable JSON protocol over stdin
and stdout; they may be written in any language.

## Manifest

`.rig/rig.toml` — the human-authored declaration of what the project
wants. TOML. See [the schema](./architecture.md#manifest-schema).

## Lockfile

`.rig/rig.lock` — the machine-authored record of what is actually installed,
with pinned SHAs. Commit this alongside `rig.toml`. TOML.

## Registry

Where bundles live. M1 = GitHub repositories (zero infrastructure). M2 =
a hosted Rig registry (`rig.dev`) with search, ratings, analytics,
private/org registries.
