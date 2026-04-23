# Rig

One tool to install, pin, sync, and share agent coding context — skills,
MCP servers, rules, commands, subagents — across Claude Code and Codex.

[Docs](./docs/introduction.md) · [Architecture](./docs/architecture.md) ·
[Concepts](./docs/concepts.md) · [Roadmap](./docs/roadmap.md) ·
[Decisions](./docs/DECISIONS.md)

## Why

- **Cross-agent.** One `rig.toml`, same stack on Claude Code and Codex
  (more adapters coming via the plugin protocol).
- **Drift-safe.** Three-SHA drift model, six states, five explicit
  resolution modes. Rig never overwrites local edits silently.
- **Reproducible.** `rig.lock` pins the install-time SHA per unit, per
  agent, per scope — bit-exact across machines.
- **Two surfaces.** A fast CLI and a Tauri-based GUI sit on the same
  core.

## Install

M1 ships as source. Pre-built binaries come later.

```sh
git clone https://github.com/griflet/rig
cd rig
cargo install --path crates/rig-cli
rig --version
```

## Quick start

```sh
rig init                                    # create ./.rig/
rig install ./my-skill --agent claude,codex # install to both agents
rig list                                    # what's installed
rig status                                  # drift state per unit
```

## CLI reference

Alphabetical; every command accepts `--scope global|project|local`
(default depends on context).

### `rig disable`

Disable a unit without uninstalling it. Works where the host agent
supports gating (Claude commands, skills with `.disabled` suffix).

```sh
rig disable claude skill react-review
```

### `rig doctor`

Scan for duplicates across scopes, broken symlinks, and stale lockfile
entries. Pass `--fix` to auto-reconcile what can be fixed safely.

```sh
rig doctor --fix
```

### `rig enable`

Re-enable a previously disabled unit.

```sh
rig enable claude skill react-review
```

### `rig init`

Create `.rig/rig.toml` and `.rig/rig.lock` in the current directory.
Detects git repos and defaults to project scope.

```sh
rig init
```

### `rig init-skill`

Scaffold a new skill directory (`SKILL.md` + starter frontmatter) ready
to be `rig install`-ed.

```sh
rig init-skill my-new-skill
```

### `rig install`

Install a unit from a source into one or more agents. Source forms:
local path, tarball, HTTP(S) URL, `github:owner/repo[@ref][#path]`.

```sh
rig install ./my-skill --agent claude,codex
rig install github:acme/react-review@v1.2 --agent claude
```

### `rig link`

Dev-loop helper: symlink a local path into an agent's native location
so edits show up live without re-installing.

```sh
rig link ./my-skill --agent claude
```

### `rig list`

List installed units by agent / type / name with drift state.

```sh
rig list
rig list --scope global --agent claude
```

### `rig mv`

Move an installed unit between scopes (e.g. promote a project skill to
global). Preserves the install-time SHA.

```sh
rig mv skill react-review project global
```

### `rig pack`

Bundle a local unit directory into a `.rig` tarball for offline share
or distribution.

```sh
rig pack ./my-skill -o my-skill.rig
```

### `rig search`

Substring search across installed units in the current scope.

```sh
rig search review
```

### `rig stats`

Per-agent, per-unit-type counts and on-disk size.

```sh
rig stats
```

### `rig status`

Show drift state per unit — `Clean / LocalDrift / UpstreamDrift /
BothDrift / Missing / Orphan`. Exits non-zero if anything is dirty, so
it's CI-friendly.

```sh
rig status
```

### `rig sync`

Read `.rig/rig.toml`, reconcile against the lockfile and disk. Pick a
drift policy with `--on-drift`: `keep`, `overwrite`, `diff-per-file`,
`snapshot-then-overwrite`, `cancel`.

```sh
rig sync --on-drift snapshot-then-overwrite
```

### `rig uninstall`

Remove a unit from an agent. Leaves manifest entry intact; use
`rig sync` to reconcile the manifest too.

```sh
rig uninstall claude skill react-review
```

### `rig unlink`

Undo `rig link`: remove the symlink and, optionally, re-install the
unit from source.

```sh
rig unlink claude skill my-skill
```

## Source types

| Kind | Form | Status |
|------|------|--------|
| Local path | `./path` or `local:./path` | shipped |
| Tarball | `./bundle.rig` or `tar:./bundle.rig` | shipped |
| HTTP(S) | `https://…/bundle.rig` | shipped |
| GitHub | `github:owner/repo[@ref][#subpath]` | shipped |
| Generic git | `git:https://…` | stub |
| npm | `npm:@scope/name` | stub |
| Marketplace | `marketplace:…` | stub |

## Unit types

| Type | What it is | Claude routing | Codex routing |
|------|-----------|----------------|----------------|
| **skill** | SKILL.md + body (Anthropic Agent Skills) | `~/.claude/skills/<name>/` | `~/.codex/skills/<name>/` |
| **rule** | Always-on project rule (CLAUDE.md / AGENTS.md) | appended to `CLAUDE.md` | appended to `AGENTS.md` |
| **command** | Slash-command markdown | `~/.claude/commands/<name>.md` | unsupported |
| **subagent** | Agent prompt with scope + tools | `~/.claude/agents/<name>.md` | unsupported |
| **mcp** | MCP server config | `.mcp.json` merge (stubbed) | `config.toml` merge (stubbed) |
| **hook** | Lifecycle hook | `settings.json` merge (stubbed) | n/a |
| **plugin** | Claude Code plugin manifest | `~/.claude/plugins/` (stubbed) | n/a |

## Scopes

- **global** — `~/.rig/rig.toml` + `~/.rig/rig.lock`. Writes into
  `~/.claude/…`, `~/.codex/…`.
- **project** — `./.rig/rig.toml` + `./.rig/rig.lock`. Writes into
  `<project>/.claude/…`, `<project>/.codex/…`.
- **local** — per-project override Claude uses for untrusted MCPs
  (`.claude/settings.local.json`).

Precedence for resolution: project > local > global. `rig list` flattens
all three unless `--scope` narrows it.

## Drift

Per unit, per agent, per scope, Rig tracks three SHAs:

```
install-sha   — what we wrote (from rig.lock)
current-sha   — what's on disk right now
upstream-sha  — what the source now offers
```

Six states: `Clean / LocalDrift / UpstreamDrift / BothDrift / Missing /
Orphan`. Five resolution modes via `rig sync --on-drift`:

- `keep` — leave local drift alone, skip upstream.
- `overwrite` — clobber local with upstream.
- `diff-per-file` — interactive TUI per file (CLI only).
- `snapshot-then-overwrite` — rename files to `.rig-backup-<ts>`,
  then write upstream.
- `cancel` — abort on first dirty state.

## GUI

```sh
cd crates/rig-gui
npm install
npm run tauri dev
```

The desktop app has three tabs:

- **Units.** Table of installed units with drift badges, a detail
  pane showing frontmatter + body, per-type filter pills
  (`All / Skill / MCP / Rule / Command / Subagent`), and a scope
  selector with an `all` option that merges global + project + local
  with origin badges and shadow indicators.
- **Stats.** Per-agent size and count breakdown.
- **Doctor.** Duplicates, broken symlinks, stale lockfile entries,
  with one-click auto-fix where safe.

Header features an **Open project…** folder picker with a recent-
projects dropdown. Modals:

- **Install** — source input, agent picker, optional unit-type
  override.
- **Sync** — drift-mode radio group, live conflicts / skipped /
  installed counts.

## Architecture

Cargo workspace of focused crates with a strict dependency graph.
`rig-core` is pure types + traits (zero I/O). Everything else —
filesystem, sources, adapters, sync, CLI, GUI — depends on `rig-core`
only. Adapters never depend on each other. New agents = new adapter
crate (or external plugin binary). See
[docs/architecture.md](./docs/architecture.md) for the full graph
and the five extensibility seams.

## Contributing

See [docs/contributing.md](./docs/contributing.md). Branches follow
`feature/<scope>` / `fix/<scope>` / `docs/<scope>` / `rfc/<name>`;
commits are imperative mood ("Add Codex MCP converter").

## License

Dual-licensed under **MIT OR Apache-2.0**.
See [LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE).
