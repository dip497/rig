# Enable / Disable / Move

Specifies two adjacent Rig CLI features: (1) `rig enable` / `rig disable` —
soft toggle of installed units without uninstall, preserving lockfile and
drift baseline; (2) `rig mv` — move an installed unit between scopes
(`global` ↔ `project`) and/or agents. Consulted when implementing the
adapter trait extension, the CLI surface, and the drift scanner fix for
disabled-file rename suffix. Assumes [`MCP-SUPPORT.md`](./MCP-SUPPORT.md)
pins the managed-ledger + snapshot format for MCP servers.

## Goals

- Every installed unit type (except `Hook`, `Plugin` in M1) can be
  disabled and re-enabled without changing its lockfile entry.
- Disable is reversible with zero upstream re-fetch.
- Disabled units remain visible in `rig list` (tagged `[disabled]`), are
  counted in `rig stats`, and never surface as `Orphan` drift.
- `rig mv` relocates a unit between scopes atomically enough that a crash
  leaves a diagnosable, `rig doctor`-reconcilable state — never silent
  duplication.
- No cross-adapter coupling; each adapter owns the per-unit-type toggle
  mechanism.

## Non-goals

- Disabling individual agents from a multi-agent install. Use
  `rig uninstall --agent <x>` for that.
- Time-bounded / scheduled disable.
- Disabling transitive bundle members independently of their bundle.
- Hook and Plugin toggle (deferred to M2, tracked as open question O1).
- Moving between agents. `rig mv` changes scope only. Agent change = new
  install.

## Key concepts

See [`concepts.md`](./concepts.md) for `Unit`, `Scope`, `Agent`, `Adapter`,
`DriftStatus`, `UnitRef`. This spec introduces:

- **Soft-disabled** — unit is present on disk and in the lockfile but
  will not be invoked by the agent runtime.
- **Rename-suffix** — the string `.rig-disabled` appended to a file's
  extension to hide it from an agent's loader without deleting it.
- **Snapshot-disable** — for stateful units (MCP), the adapter writes
  the effective config to `<scope>/.rig/disabled/<type>/<name>.<agent>.json`
  and removes the live entry from the agent's settings.

## Disable/enable mechanism per unit type

| Unit type | Mechanism | State file | Reversible |
|-----------|-----------|------------|------------|
| `Skill` | frontmatter: set `disable-model-invocation: true` | in-place edit of `SKILL.md` | yes |
| `Mcp` | snapshot + `claude mcp remove` (Claude) / delete from `~/.codex/config.toml` `[mcp_servers.<name>]` (Codex) | `<scope>/.rig/disabled/mcp/<name>.<agent>.json` | yes |
| `Rule` | rename `<name>.md` → `<name>.md.rig-disabled` | rename only | yes |
| `Command` | rename `<name>.md` → `<name>.md.rig-disabled` | rename only | yes |
| `Subagent` | rename `<name>.md` → `<name>.md.rig-disabled` | rename only | yes |
| `Hook` | **Unsupported** in M1 | — | — |
| `Plugin` | **Unsupported** in M1 | — | — |

`rig list` output for a disabled unit:

```
skill/acme/react-review    claude,codex   project   [disabled]  clean
rule/acme/ts-strict        claude         project                clean
mcp/figma                  claude         global    [disabled]  clean
```

JSON form (`rig list --json`) adds `"disabled": true` to each affected row.

`rig stats` counts disabled units in a separate bucket:

```
units installed: 12  (enabled: 10, disabled: 2)
```

## Skill disable — frontmatter semantics

Claude Code honours `disable-model-invocation: true` natively (skill
is still `/skill-name` invocable by the user but not auto-loaded by the
model). Rig treats that key as the canonical disable signal.

Before (`~/.claude/skills/react-review/SKILL.md`):

```markdown
---
name: react-review
description: Review a React component.
---
# React Review
...
```

After `rig disable skill/acme/react-review`:

```markdown
---
name: react-review
description: Review a React component.
disable-model-invocation: true
rig-disabled-at: 2026-04-21T10:14:22Z
---
# React Review
...
```

`rig-disabled-at` is a Rig-owned key (prefixed to avoid collision), used
by `detect_drift` to recognise Rig's own edit. `rig enable` removes
both `disable-model-invocation` and `rig-disabled-at`.

Codex has no native skill-disable. Rig falls back to the rename
mechanism (`SKILL.md` → `SKILL.md.rig-disabled`) for `agent = codex`.

## MCP disable — snapshot + restore

On `rig disable mcp/figma --agent claude`:

1. Read the current effective config via `claude mcp get figma --json`.
2. Write to `<scope>/.rig/disabled/mcp/figma.claude.json`:

   ```json
   {
     "schema": "rig/v1",
     "disabled_at": "2026-04-21T10:14:22Z",
     "agent": "claude",
     "scope": "project",
     "config": {
       "name": "figma",
       "command": "npx",
       "args": ["-y", "figma-mcp"],
       "env": { "FIGMA_TOKEN": "..." }
     }
   }
   ```

3. `claude mcp remove figma --scope <scope>`.
4. Keep the managed-ledger entry from `MCP-SUPPORT.md` but mark it
   `state = "disabled"`.

On `rig enable mcp/figma --agent claude`:

1. Read `<scope>/.rig/disabled/mcp/figma.claude.json`.
2. `claude mcp add-json figma '<config>' --scope <scope>`.
3. Delete the snapshot file; flip ledger entry to `state = "enabled"`.

Codex path: remove the `[mcp_servers.figma]` block from
`~/.codex/config.toml` on disable; re-insert on enable from snapshot.

Secret redaction: the snapshot mirrors the managed-ledger's redaction
rules (`MCP-SUPPORT.md §4`). `env` values flagged as secret are stored
as `"${env:FIGMA_TOKEN}"` references, not plaintext.

## Rule / Command / Subagent disable — rename

Single mv. On-disk example for Claude:

```text
~/.claude/rules/acme/ts-strict.md
  → rig disable rule/acme/ts-strict
~/.claude/rules/acme/ts-strict.md.rig-disabled
```

No content change → install_sha vs current_sha match once the suffix is
stripped. See drift section below.

`rig enable rule/acme/ts-strict`:

```text
~/.claude/rules/acme/ts-strict.md.rig-disabled
  → ~/.claude/rules/acme/ts-strict.md
```

If the target filename already exists (user created a rule with the
same name while it was disabled), abort with exit code `21` — do not
overwrite.

## Drift scanner treatment of disabled units

The Claude and Codex adapters' `list()` and `detect_drift()`
implementations must:

1. **Enumerate `*.rig-disabled` files** when listing. Present them as
   `InstalledUnit { disabled: true, .. }`.
2. **Strip the `.rig-disabled` suffix** before computing `current_sha`.
3. **Never classify a disabled file as `Orphan`** — the rename is a Rig
   signal, not a drift signal.
4. **For frontmatter-disabled skills**, normalise the YAML block by
   removing `disable-model-invocation` and any `rig-disabled-*` keys
   before hashing. Only then compare against `install_sha`.

Pseudocode:

```rust
fn current_sha_for_drift(path: &Path, unit_type: UnitType) -> Sha {
    let bytes = read_physical(path); // follows .rig-disabled suffix
    match unit_type {
        UnitType::Skill => {
            let mut doc = parse_frontmatter(&bytes);
            doc.frontmatter.remove("disable-model-invocation");
            doc.frontmatter.retain(|k, _| !k.starts_with("rig-disabled-"));
            sha256(doc.to_bytes())
        }
        _ => sha256(&bytes),
    }
}
```

Consequence: a disabled-but-otherwise-untouched unit stays `Clean`.
A disabled unit the user then hand-edits becomes `LocalDrift`.

## `rig enable` / `rig disable` CLI

```text
rig disable <type>/<name> [--agent <a>[,<b>]] [--scope <global|project>]
rig enable  <type>/<name> [--agent <a>[,<b>]] [--scope <global|project>]
```

Resolution:

- `--agent` omitted → apply to every agent where the unit is installed
  in the resolved scope.
- `--scope` omitted → the scope the unit is currently installed in; if
  the unit exists in both, error with exit `22` and ask for `--scope`.

Examples:

```sh
rig disable skill/acme/react-review
rig disable mcp/figma --agent claude --scope project
rig enable  rule/acme/ts-strict --agent claude,codex
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0    | all requested (agent × unit) toggles applied |
| 20   | unit not installed in the resolved scope |
| 21   | enable target collides with an existing non-Rig file |
| 22   | ambiguous scope (installed in both, none specified) |
| 23   | unit type does not support toggle (Hook, Plugin) |
| 24   | adapter I/O failure (details in stderr) |

Output on success (human):

```text
disabled skill/acme/react-review
  claude  project  ~/.claude/skills/acme/react-review/SKILL.md
  codex   project  ~/.codex/skills/acme/react-review/SKILL.md.rig-disabled
```

## `rig mv` CLI + atomicity story

```text
rig mv <type>/<name> --to <global|project> [--agent <a>[,<b>]]
```

Chosen atomicity model: **(c) pre-flight + ordered steps + diagnosable
failure**, with `rig doctor` reconciliation (option b) as the cleanup
path. No two-phase commit.

Ordered sequence for each (unit, agent) pair:

1. **Pre-flight**
   - Resolve source scope (the single scope the unit currently lives in).
     If both, require user to pass `--from` (reserved flag; error 32 if
     absent).
   - Refuse if target scope already contains a unit with the same
     `type/name` for that agent (exit 31).
   - Read the source lockfile entry; stage an identical entry for the
     target scope in memory.
2. **Write target** — call `adapter.install(unit, to_scope)` using the
   cached source bytes (no re-fetch, no re-resolve).
3. **Commit target lockfile** — atomic write to
   `<to_scope>/.rig/rig.lock`.
4. **Remove source** — `adapter.uninstall(unit_ref, from_scope)`.
5. **Commit source lockfile** — atomic write to
   `<from_scope>/.rig/rig.lock`.

Crash windows:

- Between 3 and 4: unit present in both scopes. `rig doctor` detects
  duplicate `install_sha` + matching `source` across scopes, prompts
  "keep which?", removes the other.
- Between 4 and 5: source scope's adapter state is clean but lockfile
  still lists the entry. `rig doctor` detects the lockfile-only entry
  and offers to strip it.

`rig doctor` is not in M1 scope but its reconcile rules for these two
cases are pinned here so the implementations match.

Lockfile diff for `rig mv skill/acme/react-review --to global`:

```diff
 # ./.rig/rig.lock  (project)
-[[lock]]
-id = "skill/acme/react-review"
-source = { kind = "github", repo = "acme/react-review", ref = "v1.2", sha = "a1b2c3" }
-install_sha = "7f8d"
-agent = "claude"
-scope = "project"
-path = "~/.claude/skills/acme/react-review/SKILL.md"

 # ~/.rig/rig.lock  (global)
+[[lock]]
+id = "skill/acme/react-review"
+source = { kind = "github", repo = "acme/react-review", ref = "v1.2", sha = "a1b2c3" }
+install_sha = "7f8d"
+agent = "claude"
+scope = "global"
+path = "~/.claude/skills/acme/react-review/SKILL.md"
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0    | all pairs moved |
| 30   | unit not installed in any scope |
| 31   | target scope already has a conflicting unit |
| 32   | ambiguous source scope, `--from` required |
| 33   | partial failure — some pairs moved, others left in place; stderr lists the state and suggests `rig doctor` |
| 34   | adapter I/O failure before any write |

## Adapter trait changes

Extend `rig_core::Adapter`:

> ⚠ Snapshot of `crates/rig-core/src/adapter.rs`. Update this block if
> the source changes.

```rust
pub trait Adapter {
    // ...existing methods...

    fn set_enabled(
        &self,
        unit_ref: &UnitRef,
        scope: Scope,
        enabled: bool,
    ) -> AdapterResult<()> {
        Err(AdapterError::Unsupported("enable/disable"))
    }

    fn is_enabled(
        &self,
        unit_ref: &UnitRef,
        scope: Scope,
    ) -> AdapterResult<bool> {
        Err(AdapterError::Unsupported("enable/disable"))
    }
}
```

Default impls preserve backward compatibility. `detect_drift` gains no
new method; the disabled-aware hashing happens inside the existing
impl.

`InstalledUnit` (returned by `list`) gains:

```rust
pub struct InstalledUnit {
    // ...existing fields...
    pub disabled: bool,
}
```

Per-unit-type dispatch lives inside each adapter's `set_enabled`; the
core resolver never branches on unit type.

## Lockfile + managed-ledger semantics

- **Disable/enable does NOT mutate `rig.lock`.** Lockfile captures
  resolved source + install baseline; disabled-ness is an adapter-side
  state, tracked via file rename, frontmatter key, or snapshot file.
- **`rig mv` DOES mutate `rig.lock`.** It removes the entry from the
  source scope's lock and writes an identical entry (same SHAs, new
  `scope`, new `path`) to the target scope's lock.
- **Managed ledger (MCP only, per `MCP-SUPPORT.md`):** gains a `state`
  field with values `"enabled" | "disabled"`. `rig mv` on an MCP flips
  the ledger's `scope` field; disable/enable flips `state`.

## Testing strategy

Test surfaces and required coverage:

- **Unit**: `current_sha_for_drift` normalisation — disabled skill
  hashes equal to enabled skill.
- **Adapter (Claude)**: `set_enabled(true/false)` roundtrip for each
  of Skill, Rule, Command, Subagent, Mcp. Assert no drift after
  toggle + toggle.
- **Adapter (Codex)**: same for Skill, Rule (+ rename fallback path
  for Skill since Codex has no frontmatter equivalent).
- **CLI integration**: golden tests for each exit code 20–24, 30–34.
- **`rig mv` crash-simulation**: inject panic between steps 3–4 and
  4–5, assert `rig doctor` (stub) detects the diagnosed state.
- **Fixture**: `tests/fixtures/disable-mv/` with pre-populated scope
  trees for both agents.

## Acceptance criteria

1. `rig disable skill/X` followed by `rig enable skill/X` leaves the
   unit in `Clean` drift state on every supported agent.
2. `rig list` shows disabled units with a `[disabled]` tag in the
   default output and `"disabled": true` in `--json`.
3. `rig stats` reports `enabled / disabled` bucket counts.
4. `rig mv skill/X --to global` produces identical install bytes at
   the new path and zero bytes at the old path; both lockfiles are
   updated in a single successful run.
5. A simulated crash between `rig mv` steps 3 and 4 leaves both
   scopes populated; `rig doctor` (or the documented manual
   reconcile) resolves cleanly.
6. Unsupported unit types (Hook, Plugin) error with exit 23 and a
   message pointing at open question O1.

## Open questions

- **O1**: Hook disable semantics. Hooks live in `settings.json`
  merged blocks — soft-disable needs either a per-hook `"enabled":
  false` key (requires Claude honouring it; it does not today) or a
  snapshot-and-remove flow like MCP. Defer to M2.
- **O2**: Plugin disable. Depends on `rig-plugin-host` spec, not yet
  written.
- **O3**: Should `rig mv` also support `--from`/`--to` agent? Current
  spec says no; revisit after `rig install --agent` UX settles.
- **O4**: Bulk toggle (`rig disable --all-in-bundle frontend-react`)?
  Deferred until bundle resolver lands (roadmap M1 item 5).

## Interoperation

- **Claude Code skill frontmatter**: `disable-model-invocation` is a
  documented Anthropic key. Rig writes it directly; this spec does
  not invent a Rig-only replacement.
- **Claude Code `mcp add-json` / `mcp remove`**: Rig shells out rather
  than editing `settings.json` directly to respect Claude's
  precedence rules. Same decision as [`MCP-SUPPORT.md`](./MCP-SUPPORT.md).
- **Codex `config.toml`**: Rig edits `[mcp_servers.*]` blocks with
  atomic writes from `rig-fs`.
- **`rig-disabled-*` namespace**: Rig-owned YAML key prefix for
  frontmatter round-tripping. No other project defines it.

## Versioning

- Schema version: `rig/v1`. Adds `disabled` to `InstalledUnit` and
  `state` to the MCP managed-ledger entry. Both additions are
  backward-compatible (default = `false` / `"enabled"`).
- Adapter trait additions are default-impl; existing adapter crates
  compile without changes.
- Migration: an older `rig.lock` written before this spec lands is
  read identically. An older adapter binary (plugin) that does not
  implement `set_enabled` surfaces exit 23 on disable; `rig list`
  and `rig mv` continue to work.
