# Philosophy

Design principles for Rig. When a decision is ambiguous, consult this
document first.

## 1. Distribution > spec

Rig is a distribution and management layer. It is **not** a new standard.
When a decision tempts us toward defining a new schema, check first
whether SKILL.md, AGENTS.md, MCPB, MCP, or Anthropic's Claude Code plugin
format already does the job. If yes, consume it.

## 2. Drift is the enemy

Two copies of the same stack that silently diverge are worse than one
copy that a team reconciles out loud. Rig always surfaces drift and
never overwrites without a human deciding. The cost of a silent merge
mistake — an edited skill reverted to upstream, a hand-tuned hook
clobbered — is far higher than the cost of one extra prompt.

## 3. Open seams, closed opinions

Every component is pluggable (units, adapters, sources, frontends,
commands). The first release ships one opinionated path through each
seam. Extensibility does not delay shipping.

## 4. Respect the existing ecosystem

We take a dependency on Anthropic's Claude Code plugin marketplace, on
Smithery and mcp-get for MCP discovery, on Agent Skills for SKILL.md,
and on the Linux Foundation's AGENTS.md. Rig adds what is missing;
it does not duplicate. If a project already owns a slice, we interop.

## 5. Simplicity wins the first release

When in doubt, cut scope. M1 ships with two agents, no daemon, no
signing, no hosted registry, no Windows. Every cut is a decision made
once and validated against reality before expanding.

## 6. Privacy-first

Zero telemetry by default. No usage tracking. No crash reports without
opt-in. Trust is a moat; do not spend it.

## 7. GitOps is the natural home for teams

The source of truth for a project's agent stack is a committed
`rig.toml` plus `rig.lock`. Everything else is caching. Team sync is
`git pull` followed by `rig install`. No fancier primitive is needed
until large orgs demand one.

## 8. Own the vocabulary, not the technology

Terms like *unit*, *bundle*, *stack*, *scope*, *drift*, *adapter* are
chosen to be unambiguous and searchable. We own and preserve them.
Tools underneath (TOML, JSON, subprocess IPC, git) are boring on
purpose.

## 9. Contributors make Rig real

Rig is a platform. Adapters for new agents, unit types for niche
workflows, bundles for specific stacks — most of them will come from
people who are not on the core team. We design for that from day one:
stable protocols, thorough docs, painless plugin authoring, zero
gatekeeping for well-scoped contributions.

## 10. Be honest when it is hard

If a translation is lossy (Claude subagent → Codex skill), Rig warns
the user at install time and records the downgrade in the lock. If a
bundle cannot resolve, Rig produces a diagnostic a human can act on,
not a stack trace. If a feature is not yet ready, we say "M1 / M2 /
M3"; we do not vapourware.

## 11. Two audiences, one tool

The individual dev wants `rig init` to "just work." The enterprise
platform team wants compliance, audit, and scope enforcement. Rig is
built for both by keeping the simple path default and the governance
path optional. Most commands behave the same for both.

## 12. Ship small, ship often

Prefer many small releases with atomic changes over occasional big
bangs. Every release ships a CHANGELOG entry and passes CI; nothing
else.
