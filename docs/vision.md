# Vision

> **Rig = the distribution and management layer for how AI codes with you.**
>
> Pip for agent context. Brew bundle for skills. Terraform for agent stacks.
> GitOps for the configuration that shapes every coding agent you use.

## The shift in progress

AI coding assistants used to be one-shot chat windows with opinions. In the
last 18 months they became *harnesses*: Claude Code, Codex, Cursor, Aider,
Continue, Cline, Windsurf, GSD, and others. Each harness is extensible by
dropping markdown and JSON into well-known directories: skills, MCP servers,
rules, hooks, subagents, plugins.

The industry now has:

- Skill distribution (SKILL.md; adopted by 26+ platforms)
- Rules distribution (AGENTS.md; 60k+ repos; Linux Foundation–donated)
- MCP distribution (MCPB bundle format; Smithery, Composio, mcp-get)
- Plugin distribution *for one vendor* (Anthropic's Claude Code marketplace)

What it does **not** have:

- A portable stack manifest that composes skills + MCPs + rules + hooks +
  subagents + plugins together, and pins the whole thing to a SHA.
- A cross-agent install flow: one command, identical outcome on Claude Code
  and Codex (and Cursor/Aider/GSD tomorrow).
- A drift-safe reconciliation loop that detects local edits and offers a
  humane merge path before overwriting.
- A team sync primitive where `.rig/rig.toml` in git equals "everyone on
  the team is running the same agent setup, end of story."
- A registry + marketplace for bundles (not just atomic units) that
  onboards a new dev to a stack in 30 seconds instead of 2 days.

Every one of those is necessary for agents-in-production to scale from
individual hobbyists to teams to enterprises. Rig is that layer.

## The analogy

| Infra era | Primitive | Distribution | Management |
|-----------|-----------|--------------|------------|
| Unix | packages | apt / yum / brew | package manager |
| Languages | libraries | npm / pypi / crates.io | resolver + lockfile |
| Containers | images | Docker Hub / ghcr | Helm / Kustomize |
| Cloud | services | Terraform registry | Terraform + GitOps |
| **AI coding** | skills/MCPs/rules/hooks | Rig | Rig |

Every previous infra category saw the same pattern: good primitives
proliferate, fragmentation hurts, a distribution + management layer emerges,
it becomes the way work moves.

Rig intends to be that layer for agent coding context.

## Moonshot, in one screen

- Every dev has a unique agent stack. Today it lives in their head and
  scattered configs. **Rig makes it portable, versioned, shareable, and
  composable.**
- `rig init` detects your stack and suggests curated bundles in 30 seconds.
  No more 2-day onboarding to a team's agent setup.
- `rig sync` makes Claude Code, Codex, and any future agent identical on
  your machine with one command.
- `rig.toml` committed in a repo lets an OSS contributor pick up your agent
  setup the instant they clone you.
- Teams ship internal bundles (`@acme/eng-baseline`). New-hire day: `rig
  add @acme/eng-baseline`. Done.
- Course creators and influencers publish bundles alongside their content.
  One link, not a 40-minute setup video.
- Eventually Rig hosts a registry of community bundles with analytics,
  reputation, and AI-suggested stacks based on your repo.

## Business model (open-core)

- **OSS forever:** `rig-core` + `rig-*` crates + CLI + official adapters +
  GUI, dual-licensed **MIT OR Apache-2.0**. Never rug-pulled.
- **Paid SaaS (`rig-cloud`, future):** hosted registry, team sync, secret
  broker for cloud MCPs at scale, org analytics, compliance bundles,
  private/on-prem registries.
- Open primitives. Opinionated, paid management plane.

## What Rig is explicitly not

- Not a new skill or MCP standard. We consume SKILL.md, MCPB, AGENTS.md.
- Not a replacement for the Claude Code plugin marketplace. We wrap it.
- Not an agent runtime. We ship the stack; the agent executes it.
- Not a Claude-first tool that bolts on multi-agent later. Cross-agent is a
  day-one requirement, not a feature flag.

## The long arc

If Rig works, in five years:

- "What's your Rig stack?" is a question developers ask each other.
- Every OSS repo has a `.rig/` directory next to `.github/`.
- Onboarding to a company means running one `rig install` command.
- The agent coding ecosystem stays interoperable even as dozens of new
  agents ship, because Rig is the portable middle layer that translates
  between them.

That's the bet.
