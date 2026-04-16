# CLAUDE.md

Guidance for Claude Code working inside this repository.

Rig is a Cargo workspace (Rust) for a cross-agent package manager of agent
coding context (skills, MCPs, rules, hooks, subagents, plugins). M1 targets
Claude Code and Codex.

**New session warmup:** read @docs/SESSION-START.md first. It's the
ten-minute catch-up.

For project overview and design see @README.md, @docs/architecture.md,
@docs/concepts.md, @docs/roadmap.md, @docs/philosophy.md. For the
append-only decision log (why the current design is the way it is)
see @docs/DECISIONS.md.

## Repository state

- `crates/rig-legacy/` is archived pre-pivot code. **Do not modify.**
  Reference only, excluded from default workspace build. Read it to
  understand historical behaviour; write new code in the new crates.
- Extensibility seams are load-bearing. See @docs/architecture.md for
  the full graph and the 5 seams (unit types, agent adapters, source
  backends, frontends, commands).

## Critical coding rules

- **`rig-core` has zero I/O.** No `std::fs`, no `reqwest`, no tokio
  sockets. If you reach for those in `rig-core`, you're in the wrong
  crate. I/O belongs in `rig-fs`, `rig-source`, adapter crates, or
  frontends.
- **No cross-adapter imports.** `rig-adapter-claude` must never import
  `rig-adapter-codex` and vice versa. Both depend only on `rig-core`.
- **Every new unit type** = one file in `rig-core/src/unit/` + one
  `Converter<Agent>` impl per adapter. Don't add unit-type branching
  to the resolver.
- **Every new agent** = new adapter crate (first-party) or external
  plugin binary (community). Never name a specific agent inside
  `rig-core`.
- **Never overwrite local edits silently.** Drift detection states
  (`Clean / LocalDrift / UpstreamDrift / BothDrift / Orphan / Missing`)
  must be surfaced on every reconcile; only the five resolution modes
  (`keep / overwrite / diff-per-file / snapshot-then-overwrite / cancel`)
  may touch bytes.
- **Respect existing standards.** Consume SKILL.md, AGENTS.md, MCPB,
  MCP, Claude Code plugin manifest. Do not invent a replacement for
  any of them.

## Dev commands

```sh
cargo check --workspace                                          # fast check
cargo test  --workspace                                          # all tests
cargo fmt --all --check                                          # formatting
cargo clippy --workspace --all-targets -- -D warnings            # lint
cargo build -p rig-cli                                           # build `rig`
cargo build -p rig-gui                                           # build desktop
cargo build -p rig-legacy                                        # archived code
```

The workspace **excludes `rig-legacy` from the default build**; build it
explicitly when referencing old behaviour.

## Workflow

- **Docs before code for non-trivial design.** Write or update the
  relevant `docs/*.md` spec first, reach alignment, then implement.
  Internal specs (`UNITS.md`, `MANIFEST.md`, `ADAPTER.md`,
  `LOCKFILE.md`, `DRIFT.md`, `RESOLVER.md`, `SCOPE.md`, `API.md`,
  `PLUGIN-PROTOCOL.md`, `M1-SPEC.md`, `TESTING.md`) land in `docs/`
  next to public-facing docs.
- **Use TaskCreate / TaskUpdate** for any work spanning 3+ steps or
  multiple crates. Mark tasks in-progress when starting, completed
  when done.
- **Delegate research to subagents** when the answer needs multiple
  searches or isolated context. Implementation work stays inline.

## Repository etiquette

- Branch naming: `feature/<scope>`, `fix/<scope>`, `docs/<scope>`,
  `rfc/<name>`.
- Commits in imperative mood ("Add Codex MCP converter", not "Added…").
- One logical change per PR.
- Update `CHANGELOG.md` for user-visible changes.
- Before merging: `cargo fmt --all --check && cargo clippy --workspace
  --all-targets -- -D warnings && cargo test --workspace`.
- Non-trivial design proposals go through an `rfc:`-labelled issue
  with one-week comment period before the implementation PR.
- Dual-licence: new files need no per-file header. The root `LICENSE`
  + `LICENSE-MIT` + `LICENSE-APACHE` cover everything.

## Communication style

Project lead writes Hinglish casual. When asked "kya lagta hai" /
"esa kuch" = "what do you think" / "something like this" → the ask is
for opinion + tradeoffs, not immediate implementation. Wait for an
explicit go-ahead before writing code. Match the casual tone in
replies; keep technical substance exact.

## Token Efficiency
- Never re-read files you just wrote or edited. You know the contents.
- Never re-run commands to "verify" unless the outcome was uncertain.
- Don't echo back large blocks of code or file contents unless asked.
- Batch related edits into single operations. Don't make 5 edits when 1 handles it.
- Skip confirmations like "I'll continue..." Just do it.
- If a task needs 1 tool call, don't use 3. Plan before acting.
- Do not summarize what you just did unless the result is ambiguous or you need additional input.
