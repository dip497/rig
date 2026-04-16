# Architecture

Rig is a Cargo workspace of focused crates with a strict dependency graph.
Every extension point is a seam; no single crate owns everything.

## Dependency graph

```
                       rig-core
                       ▲  (pure types + traits; no I/O)
                       │
          ┌────────────┼────────────────────┐
          │            │                    │
       rig-fs      rig-source          rig-plugin-host
     (atomic      (github/npm/    (subprocess IPC
      writes)      marketplace)    for plugins)
          │            │                    │
          └────────────┼────────────────────┘
                       │
                   rig-sync
              (drift engine;
           reconciliation)
                       │
       ┌───────────────┼──────────────────────┐
       │               │                      │
rig-adapter-     rig-adapter-         (community adapters
   claude           codex               via rig-plugin-host)
       │               │
       └───────┬───────┘
               │
           rig-api   ◄───── stable IPC contract
               │
       ┌───────┼──────────┐
       │       │          │
   rig-daemon  │          │
       │       │          │
       └───────┘          │
               │          │
         rig-cli      rig-gui     (community: rig-vscode, rig-tui,
        (clap CLI)   (Tauri)       rig-mobile … via rig-api)
```

Rules:

- `rig-core` knows nothing. Zero I/O, zero agent names hardcoded, zero
  network. Pure types and traits.
- Adapters never depend on each other.
- Adding a new agent is a new crate (official) or external binary (plugin).
  Zero edits to core or other adapters.
- Frontends swap freely on the stable `rig-api` contract.

## Crate roster

| Crate | Purpose | I/O? |
|-------|---------|------|
| `rig-core` | Unit types, bundle / manifest / lockfile schemas, Adapter and Converter traits, resolver, drift state machine | no |
| `rig-fs` | Atomic writes, symlinks, path normalisation, content hashing | yes (fs) |
| `rig-source` | Fetch github / git / npm / marketplace / local | yes (net + fs) |
| `rig-sync` | Drift classification and reconciliation | yes (fs) |
| `rig-plugin-host` | Subprocess IPC host for external plugins | yes (process) |
| `rig-adapter-claude` | Claude Code read / write / translate | yes (fs + process) |
| `rig-adapter-codex` | Codex read / write / translate | yes (fs) |
| `rig-api` | Stable JSON-RPC contract between daemon and frontends | no |
| `rig-daemon` | Optional always-on server for multi-frontend sessions | yes |
| `rig-cli` | `rig` binary, clap-based | yes |
| `rig-gui` | Tauri desktop GUI | yes |
| `rig-legacy` | Archived pre-pivot Claude-only TUI, reference only | excluded from default build |

## The five extensibility seams

| # | Seam | Add a new … | How |
|---|------|-------------|-----|
| 1 | Unit type | skill variant, typescript-config, etc. | one file in `rig-core/src/unit/` or an external `rig-unit-*` binary |
| 2 | Agent adapter | Cursor, Aider, Windsurf, GSD | new crate `rig-adapter-*` or external binary |
| 3 | Source backend | S3, internal artifactory, Smithery | one file in `rig-source/src/` or external binary |
| 4 | Frontend / UI | TUI variant, VSCode extension, mobile app | new process that speaks `rig-api` |
| 5 | Command | a domain-specific `rig foo` subcommand | `rig-<cmd>` external binary on PATH (git / cargo pattern) |

Every seam uses the same pattern: stable contract, subprocess IPC where
the extension is external, compile-time when it is first-party.

## Adapters

Every adapter implements:

```rust
pub trait Adapter {
    fn agent() -> AgentId;
    fn capabilities() -> UnitTypeSet;
    fn install(unit: &Unit, scope: Scope) -> Result<Receipt>;
    fn uninstall(unit_ref: &UnitRef, scope: Scope) -> Result<()>;
    fn list(scope: Scope) -> Result<Vec<InstalledUnit>>;
    fn read_local(unit_ref: &UnitRef, scope: Scope) -> Result<Canonical>;
    fn detect_drift(unit_ref: &UnitRef, scope: Scope) -> Result<DriftStatus>;
}
```

Per unit type, the adapter implements a `Converter<A: Agent>` that
translates the canonical unit into the native format (and back). Adding a
new unit type means one new file in `rig-core/src/unit/` plus one
`Converter` impl per adapter. Adapters declare their capabilities so Rig
warns when a unit is unsupported or lossy.

## Plugins

Community extensions live outside the workspace and communicate with Rig
over a versioned JSON-RPC protocol on stdin / stdout. Rig spawns
`rig-adapter-<name>` (or `rig-unit-<name>`, `rig-source-<name>`,
`rig-<cmd>`) as a subprocess and exchanges typed messages.

Registered plugins live at `~/.rig/plugins/` with a `plugin.toml`
manifest. The full contract is in
[`docs/plugin-protocol.md`](./plugin-protocol.md) (placeholder — coming in
M1 alongside the `rig-plugin-host` implementation).

## Manifest schema

`.rig/rig.toml` is the human-authored declaration.

```toml
schema = "rig/v1"

[project]
name = "my-app"

[agents]
targets = ["claude", "codex"]   # which hosts to install into

[scope]
default = "project"              # "project" | "global"

[bundle."frontend-react"]
skills = [
  "github:acme/react-review@v1.2",
  "github:acme/shadcn-helper",
]
mcps = [
  "smithery:figma-mcp",
]
rules = [
  "github:acme/react-ts-rules",
]
```

Bundles can include `bundles = [...]` to compose bundle-of-bundles.

## Lockfile schema

`.rig/rig.lock` records the resolved install-time SHA for every unit and
every target agent. Committing the lockfile makes `rig install` bit-exact
across machines.

Shape (TOML, one entry per `(unit, agent, scope)` tuple):

```toml
schema = "rig/v1"

[[lock]]
id = "skill/github:acme/react-review"
source = { kind = "github", repo = "acme/react-review", ref = "v1.2", sha = "a1b2c3…" }
install_sha = "7f8d…"
agent = "claude"
scope = "project"
path = "~/.claude/skills/react-review/SKILL.md"
```

## Scope resolution

Two scopes: `global` (`~/.rig/`) and `project` (`./.rig/`). Project wins
on conflict. Listing flattens both unless `--scope global|project` is set
explicitly.

Within each agent, Rig uses that agent's native scope hierarchy. For Claude
Code that means Rig honours the user / project / local / managed settings
precedence defined by Anthropic.

## Drift state machine

Per unit, per agent, per scope, Rig classifies one of:

| State | install-time ≟ current-disk | upstream ≟ install-time |
|-------|------------------------------|--------------------------|
| `Clean` | ✓ | ✓ |
| `LocalDrift` | ✗ | ✓ |
| `UpstreamDrift` | ✓ | ✗ |
| `BothDrift` | ✗ | ✗ |
| `Orphan` | on disk, not in `rig.toml` | — |
| `Missing` | in `rig.toml`, not on disk | — |

`rig sync` offers five resolutions for non-clean states: `keep`,
`overwrite`, `diff-per-file`, `snapshot-then-overwrite`, `cancel`. Nothing
writes silently.

Full design notes land in [drift.md](./drift.md) (coming in M1).

## Bundle resolution

Given a top-level set of bundles, Rig:

1. Expands `bundles = [...]` transitively (cycle-detected).
2. Intersects SemVer ranges when two sources request the same unit.
3. Reports conflicts that cannot be intersected and fails install with a
   human-readable diagnostic.
4. Writes the flattened set of units with their pinned SHAs to the lock.

See [resolver.md](./resolver.md) (coming in M1) for the algorithm and the
conflict diagnostics format.
