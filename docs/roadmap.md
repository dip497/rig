# Roadmap

Rig is pre-1.0. The following roadmap is aspirational and subject to
change. Current release line: `0.2.0-dev`.

## M1 — "Rig works, on Claude Code + Codex"

Ship criteria: a polyglot dev running both Claude Code and Codex can
declare a stack in `rig.toml`, install it across both agents, sync
drift-safely, and share the project with a teammate who reproduces it
with one command.

Included:

- Workspace + `rig-core` with all seven canonical unit types.
- `rig.toml` manifest schema + parser + JSON schema file.
- `rig.lock` lockfile with SHA pinning.
- `Adapter` and `Converter` traits.
- Bundle resolver with SemVer intersection and cycle detection.
- Drift state machine with all five resolution modes.
- Scope resolution (global / project) honouring each agent's native
  hierarchy.
- `rig-adapter-claude` with all seven unit types (Claude is the richest
  target).
- `rig-adapter-codex` with the five unit types Codex supports natively
  (skill, mcp, rule, hook, command) plus graceful downgrades for subagent
  and plugin.
- `rig init` with stack detection (package.json, Cargo.toml, pyproject.toml,
  go.mod) and bundle suggestions.
- `rig add / remove / install / sync / status / list / bundle`.
- GitHub-as-registry source backend (zero infra).
- `rig-cli` (clap) and `rig-gui` (Tauri) frontends sharing state via
  `rig-core` linked in-process.
- Plugin protocol specified and frozen; `rig-plugin-host` scaffolded.
- Full public-facing docs (this folder).
- CI on Linux + macOS.
- Dual MIT / Apache-2.0 licensing.

Deferred to M1.5 (best-effort if time allows):

- `rig-tui` official TUI frontend (ratatui).
- Windows support.
- `claude-marketplace:` source backend for plugin install.

## M2 — "Rig extends, and teams adopt"

- `rig-daemon` + full `rig-api` so multiple frontends run simultaneously.
- Plugin protocol end-to-end: `rig plugin install` fetches, verifies, and
  invokes external adapter / unit / source / command binaries.
- Adapters for Cursor, Aider, Continue, Cline, Windsurf (likely
  community-driven, Rig team curates).
- Sigstore / cosign signing for bundles and plugins.
- `SecretProvider` abstraction (1Password, HashiCorp Vault, env, GCP / AWS
  secret managers) on top of the M1 keychain default.
- Hosted bundle registry at `rig.dev`: search, ratings, popularity,
  semantic discovery ("frontend react tailwind" → recommended bundles).
- Private / org registries for paying users.
- Telemetry (strictly opt-in) feeding bundle recommendations.
- Windows support first-class.

## M3 — "Rig becomes infrastructure"

- Team sync primitive with RBAC, SSO, audit log, compliance bundles.
- Cloud MCP secret broker (the unique enterprise wedge): encrypted team
  vault, per-user credentials, rotation, revocation, audit — the thing
  Claude Code's 2KB keychain cap cannot do at scale.
- AI recommender: scan a repo's history and suggest bundles the team
  would likely want.
- `rig diff @v1 @v2` and `rig rollback` for time-travel across stack
  versions.
- Marketplace creator economy (Vercel-style 70/30 revenue split).
- VSCode extension as an official frontend.
- ANY-language adapter SDKs (Python, Node, Go) to lower the barrier for
  community adapter authors.

## Principles for deciding what lands when

- **Simplicity wins M1.** Every scope cut (no Windows, no hosted registry,
  no signing) buys ship speed to validate the thesis with real users.
- **Open primitives, paid management.** OSS surface never rug-pulled;
  paid features are net new.
- **Earn permission to grow.** No heavyweight governance, no foundation,
  no co-maintainers until the product validates.
- **Respect existing standards.** Never invent where SKILL.md, AGENTS.md,
  MCPB, or Claude plugins will do.
