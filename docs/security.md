# Security

Rig installs code and configuration into an agent's home directory. Its
threat model matters.

## Scope of Rig's security surface

Rig writes to:

- `~/.rig/` (global scope)
- `./.rig/` (project scope)
- Each target agent's home directory (e.g. `~/.claude/`, `~/.codex/`)
- The OS keychain, when storing secrets

Rig reads / fetches from:

- GitHub repositories (HTTPS)
- npm registries (HTTPS, M2+)
- Claude Code plugin marketplaces (HTTPS, via Anthropic's mechanisms)
- Local paths
- User-installed plugin binaries (subprocess)

Rig executes:

- Its own binaries (`rig`, `rig-gui`, `rig-daemon`)
- External adapter / unit / source / command plugins as subprocesses
  (user-installed)
- `claude plugin install` as a subprocess (for Claude plugin sources)

## M1 threat model

### In scope

- **Tampered source content.** Rig detects via SHA comparison between
  `rig.lock` and fetched content. Mismatch fails `install` with a
  diagnostic.
- **Silent overwrite of user edits.** Drift detection ensures Rig never
  overwrites a locally-modified unit without an explicit resolution
  choice.
- **Secrets on disk.** Secrets referenced in `rig.toml` are names, not
  values. Values live in the OS keychain. Rig's keychain access is
  scoped per project / unit where possible.
- **Dependency confusion.** `rig.toml` sources are fully qualified
  (`github:owner/repo`, `smithery:name`). No implicit precedence.

### Out of scope (M1)

- **Plugin binary signing.** M1 plugin binaries are unsigned; users install
  them at their own risk. Treat a Rig plugin the way you'd treat any
  `cargo install` or `npm install -g`. Sigstore signing lands in M2.
- **Sandbox of plugin execution.** Plugins run as the user. WASM sandbox
  is a possible M3 addition for untrusted plugins.
- **Network MITM.** Rig relies on HTTPS. No custom certificate pinning
  in M1.
- **Cloud secret brokerage.** Managing remote MCP credentials at scale
  is the enterprise wedge and arrives in M3.

## Reporting a vulnerability

If you find a vulnerability in Rig or any first-party crate, please
**do not** open a public GitHub issue. Email the maintainer
(`rig-security@` once the domain is registered; until then,
`utsav.itsm@gmail.com`) with:

- a description of the vulnerability,
- steps to reproduce,
- the affected version,
- your disclosure preferences.

We aim to acknowledge within 48 hours, patch within 14 days for
high-severity issues, and credit you in the advisory unless you prefer
otherwise.

## Your responsibilities as a user

- Review `rig.lock` diffs before you merge. Pinned SHAs changing is
  material.
- Review plugin binaries before you `rig plugin install`. Code is
  code.
- Be skeptical of community bundles that ask for broad permissions
  (many MCPs, many hooks) — especially if the bundle is new.
- Keep Rig itself up to date. `rig self-update` is the fast path.

## A note on third-party bundles and skills

Rig distributes content created by other humans. Nothing in `rig-core`
audits a skill's prompt or an MCP server's behaviour. You are
responsible for the content of any bundle you install. When in doubt,
inspect `SKILL.md`, the MCP server's source, and the rule text before
enabling. Rig can make that review easier (`rig inspect <unit>`);
it cannot replace the judgement.
