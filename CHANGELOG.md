# Changelog

All notable changes to Rig are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — `0.2.0-dev`

### Added

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
