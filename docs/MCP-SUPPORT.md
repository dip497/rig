# MCP Support

Implementation spec for MCP (Model Context Protocol) units in Rig. MCPs
are the first Rig unit type whose install target is not a file Rig owns
outright, but an entry inside an agent-managed config (`~/.claude.json`,
`.mcp.json`, `~/.codex/config.toml`). This spec defines how Rig models
an MCP canonically, how each adapter writes / reads / hashes / removes
it, and how drift is detected without clobbering user state.

Consulted when: adding MCP support to `rig-adapter-claude` and
`rig-adapter-codex`, or reviewing code that touches agent config
files.

## Goals

- Canonical `Mcp` unit type covers `stdio`, `http`, `sse` transports.
- Install and uninstall via the agent's own CLI, never by hand-editing
  config files.
- Drift detection over a normalised canonical form so whitespace /
  key-order differences in the native config don't trigger false
  `LocalDrift`.
- Per-agent scope mapping explicit, including Claude's third
  `local` scope.
- Zero secret expansion: `${VAR}` placeholders survive the round-trip
  literally.

## Non-goals

- Running / health-checking MCP servers. Rig installs the entry; the
  agent starts the server.
- Discovering MCP servers from a registry (Smithery, mcp.directory).
  That's a `Source` concern, not an adapter concern.
- Managing secret values. Rig records declared env var *names* only.
  Values live outside Rig (shell env, agent-native secret store).
- Patching user-authored MCP entries Rig did not install.

## Key concepts

- **Managed entry** — MCP entry whose `name` appears in the Rig
  lockfile. Rig touches only these.
- **Foreign entry** — MCP entry in the same config file that Rig did
  not install. Never modified, never hashed, never listed.
- **Canonical TOML** — deterministic serialisation of an `Mcp` unit
  used as the input to `install_sha`.
- **Agent CLI probe** — one-shot check at adapter construction that
  the host's MCP subcommand exists.

See `docs/concepts.md` for `Unit`, `Scope`, `DriftState`, `Receipt`.

## 1. Goal & scope

Rig manages MCP server *entries* in Claude Code and Codex config
stores. One canonical `Mcp` unit maps to one entry per target agent
per scope. Install is idempotent (same input → same on-disk state).
Uninstall is idempotent (absent entry is not an error). Drift is
computed over the canonical bytes, not the agent's native bytes.

In M1 the supported matrix is:

| Agent  | stdio | http | sse | scopes                   |
|--------|-------|------|-----|--------------------------|
| claude |  yes  | yes  | yes | global, project, local   |
| codex  |  yes  | yes  | no  | global                   |

Codex SSE and Codex project/local scope are `Unsupported` in M1.

## 2. Canonical Mcp type (Rust struct)

Already defined in `crates/rig-core/src/unit/mcp.rs`:

```rust
pub struct Mcp {
    pub name: String,
    pub description: Option<String>,   // NOT SHA-significant
    pub transport: Transport,          // SHA-significant
    pub env: Vec<String>,              // declared names only; SHA-significant (sorted)
    pub metadata: BTreeMap<String, String>, // SHA-significant
}

pub enum Transport {
    Stdio { command: String, args: Vec<String> },
    Http  { url: String, headers: BTreeMap<String, String> }, // add headers
    Sse   { url: String, headers: BTreeMap<String, String> }, // add headers
}
```

Spec additions vs current code:

- `Transport::Http` and `Sse` gain `headers: BTreeMap<String, String>`
  (Claude supports `--header`). Omit / empty when unused.
- `env: Vec<String>` is normalised as a sorted, dedup'd vector before
  hashing. Value of each env var is *never* embedded.
- `description` moved to the Rig-side ledger (§9) — Claude's CLI has
  no description field.

### Canonical TOML for hashing

`install_sha = Sha256::of(canonical_toml(mcp).as_bytes())` where
`canonical_toml` is:

```toml
name = "github"
transport = "stdio"          # always present; literal string
command = "npx"              # stdio only
args = ["-y", "@modelcontextprotocol/server-github"]  # stdio only, preserved order
url = "https://..."          # http/sse only
headers = { Authorization = "Bearer ${GH_TOKEN}" }  # sorted, omit if empty
env = ["GITHUB_TOKEN"]       # sorted ascii, omit if empty
metadata = { timeout_ms = "30000" }  # sorted, omit if empty
```

Rules:

1. Keys in a fixed order: `name, transport, command, args, url,
   headers, env, metadata`. Omitted when not applicable to the
   transport or when empty.
2. `transport` is the string tag (`"stdio" | "http" | "sse"`), never
   a table. Keeps the hash stable if we add transports later.
3. `${VAR}` placeholders are preserved byte-for-byte. No env
   expansion, ever.
4. Empty maps / vectors are *omitted*, not written as `{}` / `[]`.
   Avoids a common source of false drift where Claude normalises
   absence vs empty.
5. Line ending `\n`, no trailing whitespace, no BOM.

### Fields that are NOT SHA-significant

- `description` — not stored by any native CLI. Rig keeps it in
  managed ledger (§9). Changing it triggers *no* drift.
- Any field added under `metadata` that a specific adapter ignores —
  still hashed on the Rig side, so changing it counts as drift (Rig
  owns the canonical source of truth).

## 3. `mcp.toml` source format (user-facing)

When an MCP is distributed as a file (`local:./mcps/github.toml` or
`github:acme/mcps/github.toml`), the file schema is:

### stdio

```toml
schema = "rig/v1"
kind = "mcp"

name = "github"
description = "GitHub MCP server"

[transport]
kind = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]

env = ["GITHUB_TOKEN"]
```

### http

```toml
schema = "rig/v1"
kind = "mcp"

name = "figma"

[transport]
kind = "http"
url = "https://mcp.figma.com/"
headers = { Authorization = "Bearer ${FIGMA_TOKEN}" }
```

### sse

```toml
schema = "rig/v1"
kind = "mcp"

name = "analytics"

[transport]
kind = "sse"
url = "https://analytics.example.com/sse"
```

Parser lives in `rig-core/src/unit/mcp.rs::parse_source`. It accepts
this surface format and yields the canonical `Mcp`. Unknown top-level
keys are rejected (strict parsing) so typos surface early.

## 4. Install flow (Claude)

Shells out to `claude mcp add`. Never edits `~/.claude.json` directly.

```rust
// Pseudocode in rig-adapter-claude/src/mcp.rs
fn install_claude(mcp: &Mcp, scope: Scope) -> AdapterResult<Receipt> {
    // 1. Idempotency: if an entry with this name already exists
    //    in this scope, `claude mcp remove <name> --scope X` first.
    //    Claude silently overwrites on duplicate add; we explicitly
    //    remove so uninstall-then-reinstall is observably clean.
    run_claude(&["mcp", "remove", &mcp.name, "--scope", scope_flag(scope)?])
        .ok(); // ignore non-zero: "not found" is fine.

    // 2. Build argv from transport.
    let mut argv = vec!["mcp", "add",
        "--transport", transport_tag(&mcp.transport),
        "--scope", scope_flag(scope)?];
    for k in sorted(&mcp.env) {
        argv.extend(["--env", k]); // name only; Claude resolves value at runtime
    }
    match &mcp.transport {
        Transport::Stdio { command, args } => {
            argv.push(&mcp.name);
            argv.push("--");
            argv.push(command);
            argv.extend(args.iter().map(String::as_str));
        }
        Transport::Http { url, headers } | Transport::Sse { url, headers } => {
            for (k, v) in headers { argv.extend(["--header", &format!("{k}: {v}")]); }
            argv.extend([&mcp.name, url]);
        }
    }

    // 3. Invoke. Non-zero exit → AdapterError::Other with stderr captured.
    run_claude(&argv)?;

    // 4. Compute install_sha from canonical_toml(mcp), not from
    //    ~/.claude.json bytes.
    let install_sha = Sha256::of(canonical_toml(mcp).as_bytes());

    // 5. Path recorded in Receipt is the config file we mutated,
    //    not the (notional) entry path. Used for file-level sync locks.
    Ok(Receipt {
        unit_ref: UnitRef::new(UnitType::Mcp, mcp.name.clone()),
        agent: AgentId::new("claude"),
        scope,
        paths: vec![claude_config_path(scope)?],
        install_sha,
    })
}

fn scope_flag(s: Scope) -> AdapterResult<&'static str> {
    match s {
        Scope::Global  => Ok("user"),
        Scope::Project => Ok("project"),
        Scope::Local   => Ok("local"),
    }
}

fn claude_config_path(s: Scope) -> AdapterResult<PathBuf> {
    match s {
        Scope::Global | Scope::Local => Ok(home()?.join(".claude.json")),
        Scope::Project               => Ok(PathBuf::from(".mcp.json")),
    }
}
```

## 5. Install flow (Codex)

Analogous, with differences:

- Only `Scope::Global` is valid. `Scope::Project` / `Local` →
  `AdapterError::Unsupported(UnitType::Mcp)` with a message pointing
  at this spec.
- `Transport::Sse` → `AdapterError::Unsupported(UnitType::Mcp)`.
- Underlying config path is `~/.codex/config.toml`.
- CLI: `codex mcp add --transport stdio|http <name> -- <cmd> [args]`.

## 6. List + drift detection

### List

An adapter's `list(Scope::X)` for MCP:

1. Enumerates the agent's native store (`claude mcp list --scope X`
   for Claude; parse `~/.codex/config.toml` for Codex).
2. **Filters to managed entries only** by intersecting with the
   managed ledger (§9). Foreign entries are invisible to Rig.
3. Returns `InstalledUnit { unit_ref, scope, paths: [config_file] }`.

### Drift detection

```rust
fn detect_drift_claude(name, scope, install_time, upstream) -> (State, Shas) {
    // 1. Read the native entry via `claude mcp get <name> --scope X`
    //    (JSON output). Not found → DriftState::Missing.
    let native = claude_mcp_get(name, scope)?;       // Option<NativeEntry>

    // 2. Re-canonicalise: native JSON → our `Mcp` struct →
    //    canonical_toml → sha. This strips Claude-side normalisation
    //    noise.
    let current = native.map(|n| Sha256::of(
        canonical_toml(&native_to_canonical(n)).as_bytes()
    ));

    // 3. Classify using shared DriftShas::classify.
}
```

`native_to_canonical` must:

- Drop Claude-added empty `env: {}` / `headers: {}` (collapse to
  absent per §2 rule 4).
- Preserve `${VAR}` placeholders byte-for-byte — Claude stores these
  literally; do not resolve.
- Sort `env` names and `headers` / `metadata` map keys.
- Reject (→ `AdapterError::Other`) if Claude invented fields Rig's
  canonical form can't represent. Fail loud; don't silently lose
  data.

## 7. Uninstall flow

```rust
fn uninstall_claude(unit_ref, scope) -> AdapterResult<()> {
    // Idempotent: swallow "not found" exit codes.
    let res = run_claude(&["mcp", "remove", &unit_ref.name,
                           "--scope", scope_flag(scope)?]);
    match res {
        Ok(_) => Ok(()),
        Err(e) if e.is_not_found() => Ok(()),
        Err(e) => Err(to_other(e)),
    }
}
```

Codex uninstall mirrors this via `codex mcp remove`.

Neither touches foreign entries. Neither rewrites the whole config
file.

## 8. Scope mapping (Rig ↔ native)

| Rig `Scope`   | Claude flag  | Claude file        | Codex             |
|---------------|--------------|--------------------|-------------------|
| `Global`      | `--scope user`    | `~/.claude.json`   | `~/.codex/config.toml` |
| `Project`     | `--scope project` | `./.mcp.json`      | `Unsupported`     |
| `Local`       | `--scope local`   | `~/.claude.json`   | `Unsupported`     |

### Adding `Scope::Local`

New variant added to `rig_core::scope::Scope`:

```rust
pub enum Scope {
    Global,
    Project,
    Local, // Claude-only per-project override; gitignored
}
```

Invariants:

- `Scope::Local` is only valid in combination with `UnitType::Mcp` on
  `AgentId("claude")`.
- Any other adapter or unit type receiving `Scope::Local` returns
  `AdapterError::Unsupported(unit_type)` with message
  `"scope `local` is only supported for MCP units on claude"`.
- CLI parse: `rig install --scope local <unit>` is accepted at parse
  time; validation happens in the adapter, not clap. Rationale:
  keeps CLI free of agent-specific branching.

## 9. Managed tracking — lockfile vs ledger

**Decision: reuse the lockfile. No separate ledger.**

Rationale:

- The lockfile already keys on `(unit_type, source, agent, scope)`.
  For MCPs we need one additional fact — the agent-native entry
  *name* — which can piggyback as a new field on `LockEntry`.
- A second file (`managed.toml`) would create a two-writer problem
  on every install / sync. Single source of truth wins.
- `description` needs a home since Claude drops it. Park it on
  `LockEntry` too (under `extra`), not SHA-significant.

Schema change to `rig-core/src/lockfile.rs::LockEntry`:

```rust
pub struct LockEntry {
    pub id: String,
    pub unit_type: UnitType,
    pub source: Source,
    pub source_sha: Sha256,
    pub install_sha: Sha256,
    pub agent: AgentId,
    pub scope: Scope,
    pub path: PathBuf,

    /// Agent-native entry name, for unit types installed as entries
    /// inside an agent-managed config (MCP). `None` for file-backed
    /// units (Skill, Rule, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_name: Option<String>,

    /// Round-tripped metadata the native CLI drops (e.g. MCP
    /// description). Not SHA-significant. Keyed by convention.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}
```

Schema version stays `rig/v1` — fields are additive with `serde`
defaults. Existing lockfiles roundtrip.

## 10. Error handling

All errors flow through `AdapterError`. New specific mappings:

| Condition                                  | Variant                                  | Message fragment                               |
|--------------------------------------------|------------------------------------------|------------------------------------------------|
| `claude` binary missing on `$PATH`         | `Other`                                  | `"claude CLI not found on PATH (required for MCP install on Scope::X)"` |
| `codex` binary missing on `$PATH`          | `Other`                                  | `"codex CLI not found on PATH"`                |
| `claude mcp add` exit ≠ 0                  | `Other` (source = captured stderr)       | `"claude mcp add failed: <stderr>"`            |
| `codex` version lacks `mcp` subcommand     | `Unsupported(UnitType::Mcp)`             | `"installed codex version lacks mcp support"`  |
| `Transport::Sse` + Codex                   | `Unsupported(UnitType::Mcp)`             | `"codex does not support sse transport"`       |
| `Scope::Local` + non-claude adapter        | `Unsupported(unit_type)`                 | `"scope `local` is only supported for MCP on claude"` |
| `Scope::Project`/`Local` + Codex MCP       | `Unsupported(UnitType::Mcp)`             | `"codex MCP supports only global scope"`       |
| Unit name collision with foreign entry     | `Other`                                  | `"refusing to overwrite unmanaged MCP `<name>` — remove it manually or choose a different name"` |
| Drift: native entry has unknown fields     | `Other`                                  | `"native MCP `<name>` has fields Rig cannot represent: <list>"` |
| `read_local` for an unmanaged name         | `NotFound`                               | ledger lookup missed                           |

Binary-missing vs non-zero-exit distinguished by probing
`which claude` (or `Command::new("claude").arg("--version")`) once
before the real invocation, and returning the specific message.

## 11. Testing strategy

- **Canonical form.** Golden-test: for each transport, assert
  `canonical_toml(mcp)` equals a committed string, byte-for-byte.
  Mutating any SHA-significant field changes the hash; mutating
  `description` does not.
- **Round-trip.** `Mcp → canonical_toml → parse → Mcp` is identity.
- **Install via fake CLI.** Tests use a `CLAUDE_BIN` env override
  pointing at a test-local script that records argv and simulates
  `~/.claude.json` writes. No real `claude` binary needed.
- **Drift normalisation.** Fixture a `~/.claude.json` where Claude
  inserted `"env": {}` and reordered keys; assert Rig reports
  `Clean` not `LocalDrift`.
- **Scope matrix.** `Scope::Local` with `UnitType::Rule` →
  `Unsupported`. `Scope::Project` + Codex MCP → `Unsupported`.
- **Idempotency.** Install twice, uninstall twice; no error, final
  state matches single-install.
- **Foreign-entry safety.** Pre-seed `.mcp.json` with an entry not
  in the ledger; install a Rig MCP; assert foreign entry untouched
  and not in `list()`.
- **Probe cache.** `CodexAdapter::new()` probes once; subsequent
  `capabilities()` calls do not re-shell-out (observed by hook
  counting invocations of the test script).

## 12. Open questions / deferred

1. **Shared MCPs in `.mcp.json`** — do we need a separate `project`
   flag to opt a project MCP into VCS-committed vs gitignored? M1
   assumes `Scope::Project` means `.mcp.json` (committed); `Local`
   means per-user override. Revisit if users want finer control.
2. **Smithery / hosted registry resolution** — adding `source =
   "smithery:<id>"` to `rig-source` is a sibling RFC. Out of scope
   here.
3. **Header secret redaction in diagnostics** — current spec prints
   headers verbatim on error. Safe while `${VAR}` is the norm, but
   needs review once we allow literal values.
4. **MCPB bundles** — if the MCP Bundle format matures, Rig may
   prefer MCPB install over `claude mcp add`. Track in adjacent
   RFC, not M1.

## Interoperation

- **Claude Code CLI** — `claude mcp add|remove|list|get`
  (https://docs.anthropic.com/claude/docs/mcp). Rig respects
  Claude's scope hierarchy (`user / project / local`) verbatim.
- **Codex CLI** — `codex mcp add|remove|list`. No `user` scope.
- **MCP spec** — transports (`stdio`, `http`, `sse`) match the
  protocol spec (modelcontextprotocol.io).
- **MCPB** — not consumed in M1. If adopted in M2, MCPB archives
  become a `Source`, not a new unit type; install still routes
  through this spec.

## Versioning

`rig/v1`. Additive `LockEntry` fields (`native_name`, `extra`) land
under v1 via `serde(default)`. Any breaking change to canonical
form (new required key, re-ordering) bumps to `rig/v2` and ships a
migration.

## Acceptance criteria

- [ ] `rig-core::unit::Mcp` extended with `headers` on `Http` / `Sse`.
- [ ] `canonical_toml(&Mcp) -> String` function in `rig-core`,
      deterministic, golden-tested for each transport.
- [ ] `Scope::Local` variant added; non-claude-MCP usage returns
      `Unsupported`.
- [ ] `LockEntry` gains `native_name: Option<String>` and
      `extra: BTreeMap<String, String>`; existing lockfiles parse.
- [ ] `rig-adapter-claude` implements `install / uninstall / list /
      read_local / detect_drift` for `UnitType::Mcp` across
      `Global / Project / Local`, shelling out to `claude mcp`.
- [ ] `rig-adapter-codex` implements the same for `Global` only,
      `stdio` + `http`, with a single cached probe in
      `CodexAdapter::new()`.
- [ ] Drift detection round-trips through `native_to_canonical` so
      whitespace / empty-map differences do not trip `LocalDrift`.
- [ ] `${VAR}` placeholders survive install + read_local verbatim
      (test fixture asserts byte equality).
- [ ] Foreign entries in `.mcp.json` / `~/.claude.json` /
      `~/.codex/config.toml` are never listed, never modified.
- [ ] All error table rows (§10) have a matching unit or integration
      test.
- [ ] `cargo fmt --all --check && cargo clippy --workspace
      --all-targets -- -D warnings && cargo test --workspace`
      passes.
