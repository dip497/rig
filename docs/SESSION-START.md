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

As of `aafd5a2` on `refactor/cross-agent-pivot` (10 commits ahead, not yet pushed).

- **CLI feature-complete for core flows.** `rig init / install / sync
  / status / list / uninstall / pack / link / unlink / init-skill /
  search / stats / doctor / enable / disable / mv` all ship. Manifest
  + lockfile persist at `<scope>/.rig/`.
- **Cross-agent thesis proven live.** `rig install local:./skill
  --agent claude,codex` writes into both `~/.claude/` and
  `~/.codex/` with independent per-agent drift tracking.
- **153 tests pass, clippy clean, zero cross-adapter imports.**
  `rig-core` remains zero-I/O (hook-enforced).
- **Supported unit-type matrix:**
  - Claude Code: Skill, Rule, Command, Subagent, **MCP**
  - Codex: Skill, Rule, Command, Subagent, **MCP** (probe-cached)
  - Hook / Plugin: stubbed, return `Unsupported`
- **Sources:** local + tarball (`.rig` / `.tar.gz`) + **HTTP(S)**
  (`ureq`) + **GitHub** (shell-out to `git`, shallow clone into
  `~/.rig/cache/`). `git:` / `npm:` / `marketplace:` still stubbed.
- **Drift resolution live.** `rig sync --on-drift
  keep|overwrite|diff-per-file|snapshot-then-overwrite|cancel` —
  never silently overwrites. Default `keep`.
- **MCP via official CLIs.** `claude mcp add/remove/list/get` +
  `codex mcp add/remove/list`. Canonical TOML pins drift SHAs.
  `--scope local` for Claude per-project override (MCP only).
  Foreign MCP entries hidden from Rig's `list`.
- **Soft-disable across all unit types.** Skill = frontmatter flag
  (`disable-model-invocation`); MCP = snapshot + remove; rule /
  command / subagent = rename trick. Drift scanner normalises out
  disable edits so disabled units stay `Clean`.
- **Scope migration.** `rig mv <type>/<name> --to global|project|local`
  with install_sha preservation + disabled-state preservation +
  `rig doctor --fix` reconciliation for crash windows.
- **GUI full dashboard.** Tauri 2 + React 19 + Vite 8 + Tailwind 4.
  Three tabs (Units / Stats / Doctor). Install modal, sync modal
  (drift mode picker), enable/disable/mv buttons, search bar (⌘K
  focus), drift badges, disabled tags. 4 new Tauri commands for
  sync/search/stats/doctor. Direct-link `rig-core` + both adapters,
  no daemon (ADR-015). `npm run build` + `cargo check` + 153 tests
  all green.
- **Dogfood in place.** `.claude/agents/` has 5 subagents,
  `.claude/skills/` has 4 Rig-specific dev skills,
  `.claude/settings.json` wires 5 hooks. Spec-writers used this
  session for `docs/MCP-SUPPORT.md` + `docs/ENABLE-DISABLE-MV.md`.

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

1. **Push 10 commits to origin** — branch is 10 ahead of `main`.
2. **Hook + Plugin support** — only two unit types still stubbed.
   Hooks need settings.json merge design (risky, touches user state).
3. **Provider expansion** — asm ships 18 adapters; we have 2. Add
   Cursor / Windsurf / Aider / Cline / Copilot / Zed etc. Each is a
   new `rig-adapter-<name>` crate. Use `rig-new-adapter` dogfood skill.
4. **Security scanner** — pre-install SKILL.md audit for dangerous
   patterns (shell exec, net, creds, obfuscation). Differentiator.
5. **Registry + catalog infra** — `rig-registry` public index,
   `rig publish`, `rig browse`, catalog website.
6. **GUI polish** — drift resolution diff-per-file UI (CLI-only
   today), command palette (⌘K unit-jump), dark mode, project
   folder picker.
7. **TUI resurrection** (ADR-008 reversal?) — asm's main hook.
8. **Bundle composition + SemVer intersection** (#8) — transitive
   `bundles = [...]`, conflict diagnostics.
9. **`rig-api` + daemon** (#15) — stable JSON-RPC so community
   frontends can land without forking `rig-gui`.

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
