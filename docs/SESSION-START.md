# Session Start — Warmup

Read this first when opening a new session on Rig (or when a new
contributor joins). It is a ten-minute warmup that lets you catch up
without re-reading the entire design tree.

## What Rig is, in one sentence

A cross-agent distribution and management layer for agent coding
context (skills, MCPs, rules, hooks, subagents, plugins) targeting
Claude Code and Codex in M1, with per-project and global scope,
drift-safe sync, and pluggable extension points.

## Where we are right now

- **Pivot done.** Old Claude-only TUI code archived to
  `crates/rig-legacy/`, excluded from default build.
- **Workspace scaffolded.** 11 new crates compile clean. No business
  logic yet — only module stubs and crate manifests.
- **Public docs done.** `README.md`, 12 files in `docs/`, dual
  licensing, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`.
- **Dogfood in place.** `.claude/agents/` has 5 subagents,
  `.claude/skills/` has 4 Rig-specific development skills, and
  `.claude/settings.json` wires 5 hooks that enforce discipline.
- **Nothing implemented.** No unit type is written, no adapter does
  anything, no CLI command exists.

## The 2-minute mental model

1. **Unit** — a single installable thing (skill, MCP, rule, …).
2. **Bundle** — named composition of units.
3. **Source** — where a unit comes from (`github:owner/repo@sha`).
4. **Agent** — host that consumes the unit (Claude Code, Codex).
5. **Adapter** — crate that translates canonical unit ↔ agent native.
6. **Scope** — global (`~/.rig/`) or project (`./.rig/`).
7. **Drift** — three SHAs per unit (install / disk / upstream) →
   6 states → 5 resolution modes. Never silent.

See `docs/concepts.md` for the full vocabulary.

## Dependency graph (read `docs/architecture.md` for full version)

```
rig-core  →  rig-fs, rig-source, rig-plugin-host
                  ↓
              rig-sync
                  ↓
           adapters (claude, codex)
                  ↓
              rig-api
                  ↓
     rig-cli, rig-gui, rig-daemon
```

Zero I/O in `rig-core`. No cross-adapter imports. Ever.

## Locked design decisions

See `docs/DECISIONS.md` for the full ADR log with reasoning. The
short version:

- Name: **Rig**
- License: **dual MIT OR Apache-2.0**
- M1 agents: **Claude Code + Codex**
- M1 platforms: **Linux + macOS** (Windows M2)
- Manifest path: `./.rig/rig.toml` + `./.rig/rig.lock`
- Scope default: ask (project if git repo, else global)
- Daemon: M2 (CLI links core directly M1)
- Signing: unsigned M1 (SHA pin in lockfile), Sigstore M2
- Registry: GitHub repos M1, hosted M2
- Telemetry: zero, ever (opt-in only if future paid tier)
- Self-update: `rig self-update`, rustup-style
- TUI dropped; **CLI + GUI (Tauri)** are the two surfaces.

## Quick dev-loop commands

```sh
cargo check --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

All four must pass before merge.

## Next concrete work (as of last session)

Pending tasks (run `/task list` to see live state):

- #3 UNITS.md — canonical unit taxonomy spec
- #4 ADAPTER.md — trait contract
- #5 MANIFEST.md — rig.toml schema
- #6 LOCKFILE.md — rig.lock schema
- #7 DRIFT.md — state machine
- #8 RESOLVER.md — bundle / dependency algorithm
- #9 SCOPE.md — global / project precedence
- #10 GUI.md — Tauri UX (replaces previous TUI task)
- #11 M1-SPEC.md — feature + acceptance
- #12 TESTING.md — strategy
- #13 PLUGIN-PROTOCOL.md — subprocess IPC contract
- #15 API.md — rig-api contract for frontends

These can be written spec-first, or back-filled after implementation.
Current lean: **direct execute with minimal spec upfront, backfill
specs as the API solidifies.** See `docs/DECISIONS.md` ADR-015.

## How to act in a new session

1. Check the task list: `/task list` (see `TaskList` tool).
2. Pick an in-progress task, or the next available pending one.
3. Read only the relevant doc(s) in `docs/` — not all of them.
4. Delegate research-heavy work to the `spec-writer` subagent (it
   has Context7, Exa, and DeepWiki tools).
5. Delegate architecture review to the `architecture-guardian`
   subagent before merging any cross-crate change.
6. Use `TaskCreate` / `TaskUpdate` for any work that spans more than
   three steps.
7. If a decision is non-trivial, log it in `docs/DECISIONS.md`.

## How to communicate

The project lead writes Hinglish casual. When asked "kya lagta
hai" or "esa kuch", the ask is for opinion + tradeoffs, not
immediate code. Wait for an explicit "chalu kar de" or equivalent
before implementing.

## Subagents on hand (`.claude/agents/`)

- `architecture-guardian` — seam violation review
- `adapter-author` — scaffolds new agent adapters
- `spec-writer` — writes internal `docs/*.md` specs (research-enabled)
- `test-author` — writes tests + fixtures
- `docs-reviewer` — reviews Markdown for clarity + cross-links

## Skills on hand (`.claude/skills/`)

- `rig-new-unit-type`
- `rig-new-adapter`
- `rig-drift-scenario`
- `rig-bundle-author`

## Hooks enforced (`.claude/hooks/`)

- Block edits to `crates/rig-legacy/`
- Auto `rustfmt` on `.rs` files after Write/Edit
- Warn if rig-core gains I/O imports
- Warn if an adapter imports another adapter
- Remind to update CHANGELOG on session stop
