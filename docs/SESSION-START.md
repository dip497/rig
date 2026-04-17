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

As of `e89b4fc` on `refactor/cross-agent-pivot` (pushed to origin).

- **First CLI wedge shipped.** `rig init / install / sync / status
  / list / uninstall` all work end-to-end. Manifest + lockfile
  persist at `<scope>/.rig/`.
- **Cross-agent thesis proven live.** `rig install local:./skill
  --agent claude,codex` writes into both `~/.claude/` and
  `~/.codex/` with independent per-agent drift tracking.
- **54 tests pass, clippy clean, zero cross-adapter imports.**
  `rig-core` remains zero-I/O (hook-enforced).
- **Supported matrix right now:**
  - Claude Code: Skill, Rule, Command, Subagent
  - Codex: Skill, Rule
  - MCP / Hook / Plugin: stubbed, return `Unsupported`
- **Sources:** local only. github / git / npm / marketplace still
  return `Unsupported`. **Next wedge = github.**
- **Dogfood in place.** `.claude/agents/` has 5 subagents,
  `.claude/skills/` has 4 Rig-specific dev skills,
  `.claude/settings.json` wires 5 hooks. Hooks firing reliably;
  skills + subagents used only sparingly — use them more.

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

## Next concrete work

Per ADR-015 we direct-execute. Ordered by value:

1. **Github source** (#24) — `git2` shallow-clone into
   `~/.rig/cache/<owner>/<repo>@<sha>/`, resolve ref via `ls-remote`.
   Biggest blocker to real-world use.
2. **`rig sync` drift resolution** (#7 extension) — currently
   clobbers. Wire the five modes (keep / overwrite / diff-per-file /
   snapshot-then-overwrite / cancel).
3. **MCP + Hook support** — settings.json merge (per-agent). Risky:
   touches user state. Design careful reconciliation first.
4. **`rig-api` + `rig-gui`** (#15, #10) — Tauri frontend once the
   RPC contract is stable. Specs for these go to docs/.
5. **Bundle composition + SemVer intersection** (#8) — transitive
   `bundles = [...]`, conflict diagnostics.

Backfill docs (#11, #12, #13) only when an external reader needs
them — the code + rustdoc are the current source of truth.

## How to act in a new session

1. `git status` — make sure the branch is clean or you know why not.
2. Check the task list: `/task list`. Pick in-progress first, else
   the next pending in the priority order above.
3. Read only the relevant doc(s) in `docs/` — not all of them.
4. Use the dogfood (previous session skipped most of it):
   - `rig-new-*` skills for scaffolding new unit types / adapters.
   - `adapter-author` subagent for adapter impl.
   - `architecture-guardian` subagent before any cross-crate merge.
   - `test-author` subagent for fixture-heavy tests.
   - `spec-writer` subagent when a doc actually needs writing
     (has Context7 + Exa + DeepWiki).
   - `docs-reviewer` subagent before committing doc changes.
5. `TaskCreate` / `TaskUpdate` for work spanning 3+ steps.
6. Log non-trivial decisions in `docs/DECISIONS.md`.
7. Update `CHANGELOG.md` before each commit (the `changelog-reminder`
   hook will nag on session stop — don't ignore).

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
