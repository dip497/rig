# Decision Log

Architectural Decision Records (ADRs) for Rig. Each entry captures the
decision, the context, the alternatives considered, and the trigger
that would revisit it. Append-only — never rewrite history.

Format: `ADR-NNN · YYYY-MM-DD · <title>` then a short body.

---

## ADR-001 · 2026-04-16 · Pivot from Claude-only TUI to cross-agent manager

**Decision.** Rig pivots from a Claude-only terminal UI for skills +
MCPs to a cross-agent distribution and management layer for all unit
types (skills, MCPs, rules, hooks, subagents, plugins) across multiple
coding agents.

**Context.** Original project was a niche TUI useful only to Claude
Code users. Market research showed the gap is not a better skill
installer — Anthropic's plugin marketplace covers Claude well —
but a cross-agent layer that composes units into stacks and keeps
multiple agents in sync.

**Alternatives.**

- (A) Stay Claude-only, polish the TUI. Rejected: limited TAM, losing
  ground to Anthropic's own marketplace.
- (B) Pivot to MCP-only management. Rejected: Smithery, mcp-get, and
  Composio already cover this slice.
- (C) **Cross-agent manager (chosen).** Unique position — no
  complete offering exists today; agentopology is the nearest
  neighbour and is nascent.

**Revisit when.** Anthropic or another vendor ships a cross-agent
package manager with comparable breadth.

---

## ADR-002 · 2026-04-16 · Archive old code to `crates/rig-legacy/`, do not delete

**Decision.** Pre-pivot code (`installer.rs`, `scanner.rs`,
`store.rs`, `skills.rs`, `mcp.rs`, `migrate.rs`, `app.rs`,
`lock.rs`, `main.rs`, `events/`, `ui/`) moves intact under
`crates/rig-legacy/` and is excluded from the workspace's default
build.

**Context.** Old code encodes real edge cases (migration from `npx
skills`, Claude-specific path handling, existing bugs). Deleting
discards that institutional memory; keeping it compiled pollutes
the new workspace.

**Alternatives.** Delete entirely (rejected — loses hard-won edge
cases); keep in main crate behind feature flags (rejected — couples
old assumptions to new code).

**Revisit when.** M1 feature parity is reached and no references
have been consulted for 30 days. Remove then.

---

## ADR-003 · 2026-04-17 · M1 agents are Claude Code and Codex, not Claude + Cursor

**Decision.** M1 ships adapters for Claude Code and Codex. Cursor /
Aider / Continue / Cline / Windsurf / GSD / Copilot follow in M2
(likely community-driven via the plugin protocol).

**Context.** Initial lean was Claude + Cursor (biggest market
surface). Project lead chose Claude + Codex instead because they are
the most extension-rich coding agents today — both support skills,
subagents, MCPs, rules, hooks, and commands — which stress-tests
Rig's unit taxonomy more thoroughly. Cursor does not yet expose
most of these concepts.

**Alternatives.** Claude + Cursor (shallower unit coverage); single
agent first (wastes the whole cross-agent thesis).

**Revisit when.** M2 planning; reassess based on community adapter
contributions.

---

## ADR-004 · 2026-04-17 · Dual licence MIT OR Apache-2.0

**Decision.** All Rig crates publish under the standard Rust
ecosystem dual licence: MIT OR Apache-2.0. Files carry no per-file
licence header; the repo root holds `LICENSE`, `LICENSE-MIT`,
`LICENSE-APACHE`.

**Context.** MIT alone would match the pre-pivot state but lacks
Apache-2.0's explicit patent grant, which matters for
infrastructure-grade adoption. AGPL was considered and rejected as
toxic to corporate users.

**Alternatives.** MIT-only (rejected — no patent protection),
Apache-2.0-only (rejected — deviates from Rust ecosystem norm), dual
with AGPL (rejected — corporate hostility).

**Revisit when.** A specific enterprise adopter requires different
licensing.

---

## ADR-005 · 2026-04-17 · Keep the name "Rig"

**Decision.** Project retains the name Rig through at least v1.0.

**Context.** Existing repository, domain redirect, `install.sh`
asset, and public binary mean renaming has cost without offsetting
benefit. No trademark conflict surfaced in due diligence.

**Revisit when.** A serious trademark conflict or strong brand
feedback emerges post-launch.

---

## ADR-006 · 2026-04-17 · Manifest at `./.rig/rig.toml`, not project root

**Decision.** Rig's per-project manifest is `./.rig/rig.toml`, lock
at `./.rig/rig.lock`, rest of Rig state under `./.rig/`.

**Context.** Keeps project root clean and parallels common tool
conventions (`.github/`, `.vscode/`, `.cargo/`). One directory for
Rig-owned files makes gitignore / project hygiene easier.

**Alternatives.** `./rig.toml` (clutters root); `./rig/` (no dot;
too visible); two files at root (worse — one file scope only).

---

## ADR-007 · 2026-04-17 · Five drift resolution modes

**Decision.** `rig sync` offers five modes on drift: `keep`,
`overwrite`, `diff-per-file`, `snapshot-then-overwrite`, `cancel`.

**Context.** Silent overwrites are the enemy (`docs/philosophy.md`
principle 2). Three modes were too coarse (no way to merge
per-file); seven was too many to present in a sync TUI. Five covers
the real-world cases.

**Alternatives.** Two (keep/overwrite) — loses nuance; seven or more
— overwhelms the interactive prompt.

---

## ADR-008 · 2026-04-17 · Drop TUI from M1 surface list; ship CLI + Tauri GUI

**Decision.** The only official frontends are `rig-cli` (clap) and
`rig-gui` (Tauri). No interactive TUI (ratatui) surface ships in
M1.

**Context.** CLI covers scripting, CI, SSH, and power users. GUI
covers visual onboarding and casual users. A TUI fills the awkward
middle ground while doubling maintenance burden. Community can ship
a TUI as a frontend plugin via `rig-api`.

**Alternatives.** TUI-only (terminal-native but loses visual
onboarding); CLI + TUI + GUI (three surfaces to maintain).

**Revisit when.** Community demand for a first-party TUI is strong.

---

## ADR-009 · 2026-04-17 · Zero telemetry by default, ever

**Decision.** Rig collects no usage data by default. Any future
telemetry is strictly opt-in, announced in docs, and never tied to
feature availability.

**Context.** Infrastructure tools earn trust through transparent
behaviour. Telemetry by default (even opt-out) would undercut
Rig's privacy-first positioning.

**Revisit when.** A paid SaaS (`rig-cloud`) is introduced and an
optional analytics tier is evaluated. Spec then; do not sneak it
in.

---

## ADR-010 · 2026-04-17 · GitHub-as-registry M1, hosted registry M2

**Decision.** M1 treats GitHub (or any git remote) as the registry:
bundles live in repos, referenced by `github:owner/repo@sha`. M2
adds a hosted registry at `rig.dev` with search, ratings, and
discovery. The GitHub source remains first-class forever.

**Context.** Hosted registry is infrastructure cost without clear
demand validation. GitHub is free, familiar, and SHA-pinnable.

**Revisit when.** Community requests for discovery outpace what
GitHub provides, usually once there are ~50 public bundles.

---

## ADR-011 · 2026-04-17 · Respect existing standards, invent nothing at the atomic layer

**Decision.** Rig consumes SKILL.md (Agent Skills), AGENTS.md
(Linux Foundation), MCPB, MCP, and Claude Code's plugin manifest
format. Rig does not propose a new unit-level standard.

**Context.** Each of the above has meaningful adoption (26+
platforms / 60k+ repos / vendor-blessed / etc.). Forking any of
them invites a standards war Rig cannot win. Rig's innovation is at
the distribution + management layer, not the format layer.

**Alternatives.** Publish an "ACP" spec that supersedes each of
these (rejected — too ambitious, high political cost, Anthropic
/ LF would not adopt).

---

## ADR-012 · 2026-04-17 · Plugin extensibility via subprocess IPC, not dynamic loading

**Decision.** Community plugins (new adapters, new unit types, new
source backends, new commands) run as external binaries that speak a
versioned JSON-RPC protocol over stdin / stdout. Rust ABI-level
dynamic loading is out of scope.

**Context.** Rust's lack of a stable ABI makes dynamic-library
plugins fragile. Subprocess IPC is language-agnostic, matches
`git-*` and `cargo-*` precedent, and naturally sandboxes.

**Alternatives.** Dynamic Rust libs (brittle); WebAssembly sandbox
(heavy M1, reasonable M3 addition for untrusted plugins).

---

## ADR-013 · 2026-04-17 · Daemon is optional and lands in M2

**Decision.** M1 frontends link `rig-core` in-process. The
`rig-daemon` binary exists in the workspace but is not required;
the daemon is needed only when multiple frontends share state
simultaneously (e.g. CLI and GUI in parallel), which is an M2
scenario.

**Context.** A daemon complicates startup, adds an IPC layer, and
requires lifecycle management. M1 users run one frontend at a time.

**Revisit when.** Multi-frontend concurrent usage becomes common
(likely M2).

---

## ADR-014 · 2026-04-17 · BDFL governance pre-1.0; no foundation yet

**Decision.** Dipendra Sharma is BDFL. Issues labelled `rfc:` drive
design proposals with a one-week comment period. Co-maintainers
invited at ~100 stars / 10 real adopters. Foundation only considered
when 5+ unrelated organisations depend on Rig.

**Context.** Heavyweight governance before product-market fit kills
velocity. Foundation move is expensive and premature.

**Revisit when.** Listed triggers are met.

---

## ADR-015 · 2026-04-17 · Direct execution with minimal upfront spec; backfill specs as APIs solidify

**Decision.** Implementation of `rig-core` proceeds without writing
all internal spec docs up front. Minimal upfront spec: manifest
schema + unit struct shapes. Remaining specs (UNITS, ADAPTER,
LOCKFILE, DRIFT, RESOLVER, SCOPE, API, PLUGIN-PROTOCOL) are written
alongside code and stabilise when the relevant API does.

**Context.** Solo / small-team early-stage development benefits from
tight feedback loops. Spec-first would block implementation on 10+
long docs before any working code. Running the code reveals the API
faster than writing about hypothetical APIs.

**Alternatives.** Full spec-first (slow, likely wrong on first
pass); zero-spec (loses alignment on cross-cutting concerns like
drift / lockfile / plugin protocol).

**Revisit when.** A contract reaches the `rig-api` / plugin-protocol
boundary (both are stable surfaces that require a frozen spec
before external parties build against them).

---

## ADR-016 · 2026-04-17 · Eat own dogfood: `.claude/` setup in the Rig repo

**Decision.** The Rig repository includes five Claude Code
subagents, four Rig-specific skills, and five Claude Code hooks in
`.claude/`. They enforce Rig's architectural rules automatically
during development.

**Context.** Rig is a tool for managing this exact context. Using
the primitives ourselves validates the taxonomy, surfaces gaps, and
lets contributors onboard with tooling already active.

**Revisit when.** Rig itself is deployable (M1); at that point the
setup should be re-expressed as a `rig.toml` bundle that `rig
install`-s on any contributor's machine.

---

## ADR-017 · 2026-04-21 · GUI stack: Tauri 2 + React 19, direct-link to rig-core

**Decision.** The Rig desktop GUI is a Tauri 2 shell hosting a React 19
+ TypeScript + Vite 8 + Tailwind 4 frontend. The Tauri Rust side links
`rig-core`, `rig-adapter-claude`, and `rig-adapter-codex` directly
in-process; there is no daemon and no `rig-api` JSON-RPC hop in M1.

**Context.** Per ADR-008, GUI is one of the two M1 surfaces alongside
the CLI. Per ADR-015, we direct-execute and backfill specs later.
`jeremymcs/gsd2-config` is a shipped production reference using this
exact stack — proven ergonomic for a native config manager. The M1
scope is read-only (browse, drift-status, detail); mutating flows
(install/uninstall/sync) stay in the CLI.

**Alternatives.**

- (A) Leptos + Tauri (all Rust). Rejected: WASM bundle + slower iter
  than React for a UI-heavy surface. M2+ reconsideration only.
- (B) Svelte 5 + Tauri. Rejected: smaller community + no in-house
  reference that already ships.
- (C) Vanilla JS. Rejected: painful past ~3 screens; drift viz needs
  structured components.
- (D) Go via `rig-api` RPC from day one. Rejected: daemon itself is
  M2 (ADR-013); forcing the RPC contract now would slow the GUI
  wedge without a second frontend to justify the seam.

**Shape.** The frontend lives at `crates/rig-gui/`; the Tauri Rust
crate lives at `crates/rig-gui/src-tauri/` (Tauri convention). M1
commands: `list_agents`, `list_units`, `detect_drift`,
`read_unit_body`, `read_manifest`, `read_lockfile`, `scope_roots`.
Wire format uses `camelCase` (serde `rename_all`); TS types are
hand-mirrored (no `ts-rs` yet — revisit in M1.5).

**Revisit when.** (1) A second frontend surface lands (VS Code /
mobile / plugin) — the RPC hop becomes load-bearing and we lift the
commands into `rig-api`. (2) GUI adds mutating flows — drift
resolution UI will need per-unit progress events.

---

*To append a new ADR, copy the heading format and add at the bottom.
Do not rewrite or delete earlier entries.*
