# Changelog

All notable changes to Rig are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — `0.2.0-dev`

### Added

- **GUI: sync / search / stats / doctor.** Three new tabs (Units /
  Stats / Doctor), a search input with Cmd-K focus, and a "Sync"
  button that opens a drift-mode modal (keep / overwrite /
  snapshot-then-overwrite / cancel; diff-per-file is CLI-only).
  Stats view surfaces per-agent × per-type counts + disk usage.
  Doctor view lists duplicates, broken symlinks, and mv-reconcile
  issues with an auto-fix button.
- **GUI: enable / disable / mv buttons.** Detail pane now exposes
  toggle for `disable-model-invocation` (skills), rename-trick
  (rule/command/subagent), snapshot+remove (MCP). "Move to…"
  dropdown dispatches `rig mv` equivalent. Unit table marks
  disabled units with a `[disabled]` tag + dimmed row.
- **`rig mv <type>/<name> --to <scope>`.** Move a unit between scopes
  without losing install metadata. Preserves `install_sha` (same
  bytes = same SHA), lockfile `source`, and disabled state across
  the move. Non-atomic by design (spec: ordered best-effort, crash
  windows reconciled by `rig doctor`). See `docs/ENABLE-DISABLE-MV.md`
  §8.
- **`rig doctor` mv reconciliation.** Detects split-state (unit in
  both scopes, lockfile only has target) and stale-lock-entry
  (unit in target only, lockfile still has source) from crashed
  `rig mv` runs. `--fix` auto-drops stale entries; split-state is
  reported-only since user intent can't be inferred.
- **`rig enable` / `rig disable`.** Toggle any installed unit on/off
  without uninstalling. Uses Claude's native frontmatter flag for
  skills (`disable-model-invocation: true`); file-rename trick for
  rule/command/subagent; snapshot-and-restore for MCP entries. The
  drift scanner normalises disable-related edits so disabled units
  stay `Clean`. See `docs/ENABLE-DISABLE-MV.md`.
- **CLI hides foreign MCP entries from `list`.** Foreign MCP servers
  (added via `claude mcp add` outside Rig) are still respected at
  install/uninstall — they just don't clutter Rig's list/search/stats.
- **MCP unit support.** `rig install … --as mcp --agent claude,codex`
  installs MCP servers via the official `claude mcp` / `codex mcp`
  CLIs, with full list/status/uninstall/drift parity for skills.
  New `--scope local` for Claude per-project override (MCP only).
  Canonical TOML form pins drift SHAs deterministically. See
  `docs/MCP-SUPPORT.md`.
- **`rig link` entries are tracked in `links.toml`.** `list`, `search`,
  `stats`, and `doctor` now surface linked skills (plain-text output
  marks them `(linked)`; JSON adds a `"linked": true` field). New
  `rig unlink <type>/<name> [--agent …] [--scope …]` command removes
  both the symlink and the links.toml entry. `doctor` flags broken
  link source paths and dangling symlinks.
- **Codex adapter now supports Command + Subagent.** Full parity with
  the Claude Code adapter for the four unit types the CLI exposes
  (skill, rule, command, subagent). Command → `~/.codex/commands/<name>.md`,
  Subagent → `~/.codex/agents/<name>.md`; same frontmatter schema as
  Claude. `rig install … --agent claude,codex` now writes command /
  subagent units into both agents.
- **GitHub source:** `rig install github:owner/repo@ref#path --agent claude`
  now works. Shells out to `git ls-remote` to resolve the ref to a SHA and
  `git init / remote add / fetch --depth=1 / checkout` into
  `~/.rig/cache/github/<owner>/<repo>@<sha>/`. Optional `#subpath`
  selects a skill directory inside the repo. No heavy `git2`/`gix`
  dependency — system `git` CLI only.
- **HTTP source:** `rig install https://example.com/skill.rig --agent claude`
  now works. `Source::Http { url }` is a new variant; `rig-source` fetches
  via `ureq`, supports `.rig`/`.tar.gz`/`.tgz` archives and plain `.md`
  files, and returns a `FetchError::Http(_)` for network failures.
- **CLI v0.3 gap-close (vs asm):** new commands `link` (symlink dev
  install), `init-skill` (scaffold SKILL.md), `search` (substring over
  installed), `stats` (per-agent/scope counts + disk usage), `doctor`
  (duplicate + broken-symlink audit). `list` and `status` gain
  `--json` for scripting.
- **Rig GUI — install + uninstall from UI.** Header "+ Install" button
  opens a modal (source + type + agents + scope); detail pane gains an
  "Uninstall" button with confirm dialog. Two new Tauri commands
  (`install_unit`, `uninstall_unit`) wrap the same flow the CLI uses.
  Drift-resolve still CLI-only.
- **Rig GUI (M1 read-only dashboard):** Tauri 2 + React 19 + TypeScript
  + Vite 8 + Tailwind 4. Agent × scope × unit matrix with drift status
  visualisation; detail pane shows SHAs, paths, and body preview.
  In-process direct-link to `rig-core` + both adapters (no daemon,
  per ADR-015). See ADR-017.
- **Tarball source** (`rig pack` + `.rig` install): `rig pack <dir>`
  produces a deterministic gzipped tar (sorted entries, zeroed
  mtime/uid/gid, gzip header mtime=0) — same input dir → byte-identical
  output. `rig install ./skill.rig --agent claude,codex` extracts to a
  tempdir and installs into both. Unlocks git-less sharing via email /
  DM / S3 / pastebin. Bare filesystem paths (`./`, `/`, `~/`) now skip
  the `local:` prefix.
- **First working `rig` CLI:** `init`, `install`, `sync`, `status`,
  `list`, `uninstall`. Manifest + lockfile persisted at
  `<scope>/.rig/`; drift detected via three-SHA model across runs.
- **Cross-agent installs:** `--agent claude,codex` writes canonical
  units into both `~/.claude/` and `~/.codex/` layouts, with
  per-agent independent drift tracking.
- **`rig-core`** canonical types: seven unit structs, `Adapter` +
  `Converter<T>` traits, manifest and lockfile schemas, drift state
  machine. Zero I/O, enforced by a PostToolUse hook.
- **`rig-adapter-claude`** Skill + Rule + Command + Subagent
  converters; install/uninstall/list/read_local/detect_drift.
- **`rig-adapter-codex`** Skill + Rule converters; same adapter
  surface, fully isolated from `rig-adapter-claude` (no cross-imports).
- **`rig-fs`** atomic write primitive + content-hash helpers.
- **`rig-source`** local source fetcher; github/git/npm/marketplace
  stubbed with `Unsupported` until later wedges.
- Workspace restructure into focused crates: `rig-core`, `rig-fs`,
  `rig-source`, `rig-sync`, `rig-plugin-host`, `rig-adapter-claude`,
  `rig-adapter-codex`, `rig-api`, `rig-daemon`, `rig-cli`, `rig-gui`.
- Public-facing documentation (`docs/introduction.md`, `docs/vision.md`,
  `docs/concepts.md`, `docs/architecture.md`, `docs/roadmap.md`,
  `docs/philosophy.md`, `docs/comparison.md`, `docs/contributing.md`,
  `docs/governance.md`, `docs/security.md`, `docs/faq.md`,
  `docs/terms.md`).
- Dual MIT / Apache-2.0 licensing.
- `CODE_OF_CONDUCT.md`, `SECURITY.md`, refreshed `README.md`.

### Changed

- **`rig sync` and `rig install` detect drift; never silently overwrite.**
  New `--on-drift` flag on both commands selects the resolution mode:
  `keep` (default — safest), `overwrite`, `diff-per-file` (unified diff
  + Y/n prompt), `snapshot-then-overwrite` (rename current to
  `<path>.rig-backup-<ts>`), or `cancel` (abort the run). Prior
  behaviour was a silent clobber.
- Project scope pivoted from "Claude-only TUI" to "cross-agent
  distribution and management layer" (M1 targets Claude Code and Codex).
- Pre-pivot codebase archived to `crates/rig-legacy/` as reference only;
  excluded from the default workspace build.

### Deprecated

- Pre-0.2 `rig` binary (TUI-only, Claude-only). Replaced by `rig-cli`
  (`rig` binary, cross-agent) once M1 lands.

## [0.1.x] — pre-pivot

- Terminal UI for managing Claude AI skills and MCP servers.
- See git history and `crates/rig-legacy/` for details. No further
  releases on this line.
