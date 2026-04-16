# FAQ

### Is Rig a new agent?

No. Rig manages the **context** (skills, MCPs, rules, hooks, subagents,
plugins) that coding agents like Claude Code and Codex consume. Rig
never executes an agent loop itself.

### Does Rig replace the Claude Code plugin marketplace?

No. Rig consumes it. When a bundle points at
`claude-marketplace:someplugin@1.2`, the Claude adapter delegates to
Anthropic's `/plugin install`. Rig adds bundle composition, lockfile,
drift detection, team sync, and translation to non-Claude targets on
top.

### Why not just use Claude Code plugins everywhere?

Because "everywhere" is only Claude Code. Rig's value appears the
moment you use two or more agents. If your entire team and every dev
only ever runs Claude Code, Anthropic's marketplace is probably
enough.

### Why not just use openskills / Smithery / Composio?

Each of those is excellent at one slice: skills, MCP servers, toolkits.
None of them compose bundles across unit types, pin a stack in a
lockfile, detect drift, sync teams, or translate across agents. Rig
interops with all three (they become source backends).

### How is Rig different from agentopology?

agentopology is the nearest neighbour. It's a declarative compiler from
`.at` DSL to seven coding-agent config layouts. Rig adopts its
compile-to-many pattern, then adds package-manager semantics (install
from source, lockfile, drift detection, team sync, plugin extensibility,
GUI). Rig ⊃ agentopology in scope.

### Is Rig a standard or a tool?

A tool. Rig does not propose a new standard for the underlying
content — SKILL.md, AGENTS.md, MCPB, Claude plugin.json already
exist. `rig.toml` is a distribution manifest on top of those, like
`Brewfile` is on top of Homebrew formulae.

### What agents does Rig support?

**M1:** Claude Code, Codex. **M2:** Cursor, Aider, Continue, Cline,
Windsurf (likely community-driven). Any agent can be targeted by
writing an adapter — compiled into the workspace for first-party, or
shipped as a `rig-adapter-<name>` plugin binary for anyone else.

### Why TOML and not YAML / JSON?

TOML is familiar to Rust / Python (pyproject.toml) / Node (wrangler,
pnpm-workspace). It is strict enough to catch typos, lenient enough
for humans to edit, and has good tooling. JSON loses comments. YAML
has significant-whitespace landmines.

### Is Rig open source?

Yes. Core crates are dual-licensed **MIT OR Apache-2.0**. Future
paid features (hosted registry, team sync, enterprise secret
broker) will be separate repositories with a separate commercial
licence. Core is never rug-pulled.

### Do you collect telemetry?

No, not by default, not ever unless the user explicitly opts in.
Privacy is a design commitment, not a toggle.

### Windows support?

**M2.** M1 targets Linux and macOS. Windows means path normalisation,
symlink vs junction vs copy handling, PowerShell vs cmd tests, and a
larger test matrix than the early project can afford.

### Can I use Rig without a GUI?

Yes. The `rig` CLI is fully featured. The Tauri GUI is optional and
talks to the same core via linked crates (or, in M2, via the daemon).

### Can I extend Rig without contributing to the main repo?

Yes — that's the point of the plugin protocol. New adapters, unit
types, source backends, and commands can all ship as external binaries
that Rig invokes via JSON-RPC over stdio. Any language that can read
stdin and write stdout will work.

### Where do bundles live?

**M1:** in GitHub repositories. A `rig.toml` plus a directory of
units is a bundle; adding `[bundle."name"]` references to another repo
makes it a bundle. **M2:** a hosted registry at `rig.dev` will add
search and discovery. The GitHub source remains supported forever.

### What happens if upstream changes a bundle while I have it installed?

Rig's drift detector notices on `rig sync` and reports
`UpstreamDrift`. You choose what to do: `keep` (ignore the update),
`overwrite` (accept it), `diff-per-file` (review each file), or
`snapshot-then-overwrite` (save your local copy before updating).
Nothing happens silently.

### I broke my setup. Can I reset?

`rig install --force` reinstalls every unit from its pinned SHA in
`rig.lock`. `rig reset --scope project` wipes local state for the
project and starts over. `rig snapshot` / `rig restore` is planned
for M2 for full time-travel.

### Is there a Discord / Slack?

Not yet. Start with GitHub issues and discussions. A Discord will
open when the community is large enough to need one (~300 users).
