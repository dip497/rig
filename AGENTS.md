# AGENTS.md

Rules for any coding agent working inside this repository (Codex, Claude
Code, Cursor, Continue, Cline, Windsurf, Aider, others). Self-contained
— no external references required.

## Project

Rig is a Cargo workspace (Rust) for a cross-agent package manager of agent
coding context — skills, MCPs, rules, hooks, subagents, plugins. M1
targets Claude Code and Codex.

Public documentation lives under `docs/`:
- `introduction.md`, `vision.md`, `concepts.md` — what Rig is.
- `architecture.md` — dependency graph, extensibility seams, trait
  contracts.
- `roadmap.md`, `philosophy.md`, `comparison.md` — positioning.

## Workspace layout

```
crates/
├── rig-core/              canonical types + traits + resolver — NO I/O
├── rig-fs/                atomic writes, symlinks, path normalisation
├── rig-source/            github / git / npm / marketplace / local fetch
├── rig-sync/              drift engine + reconciliation
├── rig-plugin-host/       subprocess IPC for external plugins
├── rig-adapter-claude/    Claude Code converter (7 unit types)
├── rig-adapter-codex/     Codex converter (5 unit types + 2 downgrades)
├── rig-api/               stable IPC contract for daemon ↔ frontends
├── rig-daemon/            optional bg server (binary)
├── rig-cli/               `rig` command (binary)
├── rig-gui/               Tauri desktop app (binary)
└── rig-legacy/            archived pre-pivot code; excluded from default build
```

`crates/rig-legacy/` is **reference only — do not modify.** Build it
explicitly with `cargo build -p rig-legacy` if you need to consult
previous behaviour.

## Critical coding rules

1. **`rig-core` has zero I/O.** No filesystem, no network, no
   subprocess. If the change needs bytes from disk or bytes over the
   wire, it belongs in `rig-fs`, `rig-source`, an adapter, or a
   frontend — not in `rig-core`.
2. **No cross-adapter imports.** `rig-adapter-claude` never imports
   `rig-adapter-codex`, and vice versa. Both depend only on `rig-core`
   and `rig-fs`.
3. **Unit types extend in one place.** A new unit type = one file
   under `rig-core/src/unit/` plus one `Converter` impl per adapter.
   Never branch on unit type inside resolver or sync code.
4. **Agents extend by crate or plugin, not by core change.** A new
   first-party agent is a new `rig-adapter-*` crate. A community
   agent is an external binary speaking the plugin protocol. Never
   name a specific agent inside `rig-core`.
5. **Drift is never silent.** Every reconciliation classifies one of
   `Clean / LocalDrift / UpstreamDrift / BothDrift / Orphan / Missing`
   and resolves via one of `keep / overwrite / diff-per-file /
   snapshot-then-overwrite / cancel`. Byte-level writes only happen
   through those modes.
6. **Respect existing standards.** Rig consumes SKILL.md, AGENTS.md,
   MCPB, MCP, Claude Code plugin manifests. Do not invent
   replacements.
7. **Pre-1.0 API churn is allowed** in `rig-core`, `rig-api`, and the
   plugin protocol, but record every breaking change in
   `CHANGELOG.md`. Stable surfaces are only the things shipped in a
   tagged release.

## Dev commands

```sh
cargo check --workspace
cargo test  --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p rig-cli              # the `rig` binary
cargo build -p rig-gui              # the Tauri desktop app
cargo build -p rig-legacy           # archived pre-pivot code
```

All four checks (`check`, `test`, `fmt --check`, `clippy -D warnings`)
must pass before a PR is merged.

## Workflow

- **Docs before code.** For design changes touching a new unit type,
  a new adapter, the manifest schema, lockfile format, drift state
  machine, or the plugin protocol, write or update the relevant
  `docs/*.md` spec first. Reach alignment. Then implement.
- **Plan for multi-crate changes.** Any change touching more than
  three crates should be captured as a task list or short plan
  before editing starts. If the plan reveals the work is actually
  two or three independent changes, split.
- **One logical change per commit and per PR.** Bundled changes are
  harder to review and revert.
- **Branches:** `feature/<scope>`, `fix/<scope>`, `docs/<scope>`,
  `rfc/<name>`.
- **Commits in imperative mood:** "Add Codex MCP converter", not
  "Added Codex MCP converter".
- **Update `CHANGELOG.md`** for any user-visible change.
- **Non-trivial proposals:** open an `rfc:`-labelled issue with
  problem / proposal / alternatives / migration. One-week comment
  period before implementation PR.

## Communication style (maintainer preference)

Project lead (Dipendra) writes Hinglish casual. Requests like "kya
lagta hai" or "esa kuch" mean "what do you think" / "something like
this" and call for opinion + tradeoffs rather than immediate
implementation. Wait for explicit go-ahead before coding. Mirror the
casual tone in responses while keeping technical substance exact.

## Licensing

Dual MIT OR Apache-2.0 at the user's option. New source files need
no per-file licence header; the root `LICENSE`, `LICENSE-MIT`, and
`LICENSE-APACHE` cover the tree. Third-party code merged in must be
compatible with both licences.

## Token Efficiency

- Never re-read files you just wrote or edited. You know the contents.
- Never re-run commands to "verify" unless the outcome was uncertain.
- Don't echo large blocks of code or file contents unless asked.
- Batch related edits into single operations. Don't make 5 edits when 1
  handles it.
- Skip confirmations like "I'll continue...". Just do it.
- If a task needs 1 tool call, don't use 3. Plan before acting.
- Do not summarise what you just did unless the result is ambiguous or
  you need additional input.
