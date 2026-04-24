//! `rig` CLI — install / sync / status / list / uninstall for Claude
//! Code. Manifest + lockfile live at `<scope>/.rig/`.

mod store;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

use rig_adapter_claude::{
    ClaudeAdapter, CommandConverter, MCPConverter as ClaudeMCPConverter, RuleConverter,
    SkillConverter, SubagentConverter,
};
use rig_adapter_codex::CodexAdapter;
use rig_core::adapter::{Adapter, InstalledUnit, Receipt, UnitRef};
use rig_core::converter::{Converter, NativeLayout};
use rig_core::drift::DriftState;
use rig_core::lockfile::{LockEntry, Lockfile};
use rig_core::manifest::Bundle;
use rig_core::scope::Scope as CoreScope;
use rig_core::source::Source;
use rig_core::unit::{Unit, UnitType};

const DEFAULT_BUNDLE: &str = "default";

#[derive(Parser)]
#[command(
    name = "rig",
    version,
    about = "Cross-agent package manager for agent coding context"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an empty `.rig/rig.toml` at the given scope.
    Init {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
    },
    /// Install a unit from a source into one or more agents.
    Install {
        source: String,
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
        #[arg(long = "as", value_enum)]
        as_type: Option<CliUnitType>,
        /// Target agent(s). Comma-separated. Default: claude.
        #[arg(long, value_delimiter = ',', default_value = "claude")]
        agent: Vec<CliAgent>,
        /// Skip adding the source to `rig.toml`. Useful for one-off installs.
        #[arg(long)]
        no_manifest: bool,
        /// How to handle local drift when re-installing over an
        /// existing unit. Defaults to `keep` (never silently overwrite).
        #[arg(long = "on-drift", value_enum, default_value_t = OnDrift::Keep)]
        on_drift: OnDrift,
    },
    /// Install everything declared in `rig.toml`, writing the lockfile.
    Sync {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
        /// Drift resolution mode (see `install --on-drift`).
        #[arg(long = "on-drift", value_enum, default_value_t = OnDrift::Keep)]
        on_drift: OnDrift,
    },
    /// Report drift between `rig.lock` and disk.
    Status {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// List Rig-installed units for Claude Code.
    List {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Uninstall a unit by `<type>/<name>`, e.g. `skill/my-skill`.
    Uninstall {
        target: String,
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
    },
    /// Pack a unit directory into a deterministic `.rig` tarball for
    /// git-less sharing. Output is byte-identical across runs.
    Pack {
        path: PathBuf,
        /// Output archive path. Defaults to `<dirname>.rig` in CWD.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Symlink a local skill directory into one or more agents (dev
    /// mode). Does not touch the manifest or lockfile.
    Link {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
        /// Target agent(s). Comma-separated. Default: claude.
        #[arg(long, value_delimiter = ',', default_value = "claude")]
        agent: Vec<CliAgent>,
        /// Overwrite any existing directory or symlink at the target.
        #[arg(long)]
        force: bool,
    },
    /// Scaffold a new skill directory with a valid SKILL.md stub.
    InitSkill {
        name: String,
        /// Parent directory to create `<name>/` in. Defaults to CWD.
        #[arg(long = "in")]
        in_dir: Option<PathBuf>,
    },
    /// Scaffold a new MCP server definition (`<name>/mcp.toml`).
    InitMcp {
        name: String,
        /// Parent directory to create `<name>/` in. Defaults to CWD.
        #[arg(long = "in")]
        in_dir: Option<PathBuf>,
        /// Transport template: stdio (default), http, or sse.
        #[arg(long, value_enum, default_value_t = CliMcpTransport::Stdio)]
        transport: CliMcpTransport,
    },
    /// Preview a unit from any source (local, tarball, http, github)
    /// without installing it. Safe read-only operation; never touches
    /// the manifest, lockfile, or any agent directory.
    Inspect {
        source: String,
        /// Emit structured JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
        /// Print the exact bytes of one file from the fetched layout
        /// to stdout. Useful for piping / diffing.
        #[arg(long = "raw")]
        raw: Option<String>,
        /// Override unit-type detection (skill|rule|command|subagent|mcp).
        #[arg(long = "as", value_enum)]
        as_type: Option<CliUnitType>,
    },
    /// Substring search across installed units. Matches name + type +
    /// frontmatter description by default; pass `--full` to also scan
    /// body bytes.
    Search {
        query: String,
        #[arg(long, value_enum, default_value_t = CliScopeAll::All)]
        scope: CliScopeAll,
        #[arg(long)]
        json: bool,
        /// Also scan unit body. Ignored when the query is shorter than
        /// 3 characters (avoid pathological full scans).
        #[arg(long)]
        full: bool,
    },
    /// Per-agent × scope breakdown: counts + disk usage.
    Stats {
        #[arg(long, value_enum, default_value_t = CliScopeAll::All)]
        scope: CliScopeAll,
        #[arg(long)]
        json: bool,
    },
    /// Remove a `rig link` symlink and drop its entry from links.toml.
    Unlink {
        target: String,
        /// Target agent(s). If omitted, removes across all agents.
        #[arg(long, value_delimiter = ',')]
        agent: Option<Vec<CliAgent>>,
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
    },
    /// Audit: duplicates across agents, broken symlinks, and `rig mv`
    /// reconciliation (split-state / stale-lock-entry).
    Doctor {
        /// Auto-resolve fixable inconsistencies (currently: drop stale
        /// lock entries from crashed `rig mv` runs). Split-state is
        /// reported only since user intent is ambiguous.
        #[arg(long)]
        fix: bool,
    },
    /// Soft-disable an installed unit. Drift stays `Clean`; lockfile
    /// is not mutated. See `docs/ENABLE-DISABLE-MV.md`.
    Disable {
        target: String,
        /// Target agent(s). Comma-separated. If omitted, every agent
        /// that currently has the unit installed in the resolved scope.
        #[arg(long, value_delimiter = ',')]
        agent: Option<Vec<CliAgent>>,
        #[arg(long, value_enum)]
        scope: Option<CliScope>,
    },
    /// Re-enable a previously disabled unit.
    Enable {
        target: String,
        #[arg(long, value_delimiter = ',')]
        agent: Option<Vec<CliAgent>>,
        #[arg(long, value_enum)]
        scope: Option<CliScope>,
    },
    /// Move an installed unit between scopes. Preserves `install_sha`
    /// and the disabled state. Non-atomic by design — `rig doctor`
    /// reconciles crash windows. See `docs/ENABLE-DISABLE-MV.md` §8.
    Mv {
        target: String,
        /// Target scope to move into.
        #[arg(long = "to", value_enum)]
        to: CliScope,
        /// Agent(s) to move. Comma-separated. If omitted, every agent
        /// that currently has the unit installed.
        #[arg(long, value_delimiter = ',')]
        agent: Option<Vec<CliAgent>>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum CliScope {
    Global,
    Project,
    /// Claude-only per-project override, MCP units only. Validated
    /// inside the adapter; see `docs/MCP-SUPPORT.md` §8.
    Local,
}

impl From<CliScope> for CoreScope {
    fn from(s: CliScope) -> Self {
        match s {
            CliScope::Global => Self::Global,
            CliScope::Project => Self::Project,
            CliScope::Local => Self::Local,
        }
    }
}

/// Scope selector that admits "all".
#[derive(Copy, Clone, ValueEnum)]
enum CliScopeAll {
    Global,
    Project,
    Local,
    All,
}

impl CliScopeAll {
    fn scopes(self) -> Vec<CoreScope> {
        match self {
            Self::Global => vec![CoreScope::Global],
            Self::Project => vec![CoreScope::Project],
            Self::Local => vec![CoreScope::Local],
            Self::All => vec![CoreScope::Global, CoreScope::Project, CoreScope::Local],
        }
    }
}

#[derive(Copy, Clone, ValueEnum, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CliAgent {
    Claude,
    Codex,
}

impl CliAgent {
    fn adapter(self) -> Box<dyn Adapter> {
        match self {
            Self::Claude => Box::new(ClaudeAdapter::new()),
            Self::Codex => Box::new(CodexAdapter::new()),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Claude => rig_adapter_claude::AGENT_ID,
            Self::Codex => rig_adapter_codex::AGENT_ID,
        }
    }
}

/// Resolution strategy when re-installing over a locally-drifted unit.
#[derive(Copy, Clone, ValueEnum, Debug)]
enum OnDrift {
    /// Leave the local version alone; skip the write.
    Keep,
    /// Overwrite after copying pre-install bytes to
    /// `<scope>/.rig/snapshots/<ts>/<agent>/<type>/<name>/`. The path
    /// is printed so recovery is one `cp` away.
    Overwrite,
    /// Show a unified diff and prompt for confirmation.
    DiffPerFile,
    /// Alias for `overwrite` — both now snapshot before writing. Kept
    /// so existing scripts keep working.
    SnapshotThenOverwrite,
    /// Abort the entire run.
    Cancel,
}

#[derive(Copy, Clone, ValueEnum)]
enum CliMcpTransport {
    Stdio,
    Http,
    Sse,
}

#[derive(Copy, Clone, ValueEnum)]
enum CliUnitType {
    Skill,
    Rule,
    Command,
    Subagent,
    Mcp,
}

impl From<CliUnitType> for UnitType {
    fn from(t: CliUnitType) -> Self {
        match t {
            CliUnitType::Skill => Self::Skill,
            CliUnitType::Rule => Self::Rule,
            CliUnitType::Command => Self::Command,
            CliUnitType::Subagent => Self::Subagent,
            CliUnitType::Mcp => Self::Mcp,
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,rig=info".into()),
        )
        .compact()
        .init();

    match Cli::parse().command {
        Command::Init { scope } => init(scope.into()),
        Command::Install {
            source,
            scope,
            as_type,
            agent,
            no_manifest,
            on_drift,
        } => install(
            &source,
            scope.into(),
            as_type.map(Into::into),
            &agent,
            !no_manifest,
            on_drift,
        ),
        Command::Sync { scope, on_drift } => sync(scope.into(), on_drift),
        Command::Status { scope, json } => status(scope.into(), json),
        Command::List { scope, json } => list(scope.into(), json),
        Command::Uninstall { target, scope } => uninstall(&target, scope.into()),
        Command::Pack { path, out } => pack(&path, out.as_deref()),
        Command::Link {
            path,
            scope,
            agent,
            force,
        } => link(&path, scope.into(), &agent, force),
        Command::InitSkill { name, in_dir } => init_skill(&name, in_dir.as_deref()),
        Command::InitMcp {
            name,
            in_dir,
            transport,
        } => init_mcp(&name, in_dir.as_deref(), transport),
        Command::Inspect {
            source,
            json,
            raw,
            as_type,
        } => inspect(&source, json, raw.as_deref(), as_type.map(Into::into)),
        Command::Search {
            query,
            scope,
            json,
            full,
        } => search(&query, scope, json, full),
        Command::Stats { scope, json } => stats(scope, json),
        Command::Unlink {
            target,
            agent,
            scope,
        } => unlink(&target, agent.as_deref(), scope.into()),
        Command::Doctor { fix } => doctor(fix),
        Command::Disable {
            target,
            agent,
            scope,
        } => toggle(&target, agent.as_deref(), scope, false),
        Command::Enable {
            target,
            agent,
            scope,
        } => toggle(&target, agent.as_deref(), scope, true),
        Command::Mv { target, to, agent } => mv(&target, to.into(), agent.as_deref()),
    }
}

fn pack(path: &std::path::Path, out: Option<&std::path::Path>) -> Result<()> {
    if !path.is_dir() {
        bail!("`{}` is not a directory", path.display());
    }
    let default_out;
    let out = match out {
        Some(p) => p,
        None => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unit".into());
            default_out = PathBuf::from(format!("{name}.rig"));
            &default_out
        }
    };
    rig_fs::pack_dir(path, out).with_context(|| format!("packing `{}`", path.display()))?;
    let bytes = std::fs::read(out).with_context(|| format!("reading `{}`", out.display()))?;
    let sha = rig_core::source::Sha256::of(&bytes);
    println!("packed {} → {}", path.display(), out.display());
    println!("  size:   {} bytes", bytes.len());
    println!("  sha256: {sha}");
    Ok(())
}

fn init(scope: CoreScope) -> Result<()> {
    let p = store::manifest_path(scope)?;
    if p.exists() {
        println!("{} already exists", p.display());
        return Ok(());
    }
    let m = store::empty_manifest();
    store::save_manifest(scope, &m)?;
    println!("initialised {}", p.display());
    Ok(())
}

fn install(
    source_ref: &str,
    scope: CoreScope,
    as_type: Option<UnitType>,
    agents: &[CliAgent],
    persist: bool,
    on_drift: OnDrift,
) -> Result<()> {
    let source =
        Source::parse(source_ref).with_context(|| format!("parsing source `{source_ref}`"))?;
    let (unit, fetched_sha) = fetch_unit(&source, as_type, source_ref)?;

    let mut any_ok = false;
    for &ag in agents {
        let adapter = ag.adapter();
        if !adapter.capabilities().contains(&unit.unit_type()) {
            println!(
                "  ⚠  {} does not support {:?}; skipped",
                adapter.agent(),
                unit.unit_type()
            );
            continue;
        }

        let name = canonical_name(&unit);
        let unit_ref = UnitRef::new(unit.unit_type(), name.clone());

        // Look up a prior install_sha from the lockfile, if any.
        let prior_install_sha = prior_install_sha(scope, unit.unit_type(), &source, ag.id());

        let Some(receipt) = apply_with_drift_resolution(
            &*adapter,
            &unit,
            &unit_ref,
            scope,
            prior_install_sha,
            on_drift,
        )?
        else {
            continue;
        };

        println!(
            "installed {}/{} into {} ({scope})",
            type_slug(receipt.unit_ref.unit_type),
            receipt.unit_ref.name,
            adapter.agent(),
        );
        for p in &receipt.paths {
            println!("  + {}", p.display());
        }
        println!("  source_sha:  {fetched_sha}");
        println!("  install_sha: {}", receipt.install_sha);
        if persist {
            upsert_lock(scope, &source, fetched_sha.clone(), &receipt)?;
        }
        any_ok = true;
    }
    if persist && any_ok {
        add_to_manifest(scope, &source, unit.unit_type(), agents)?;
        println!("  rig.toml + rig.lock updated");
    }
    Ok(())
}

/// Return the canonical unit name (the one the adapter uses as path stem).
fn canonical_name(unit: &Unit) -> String {
    match unit {
        Unit::Skill(u) => u.name.clone(),
        Unit::Rule(u) => u.name.clone(),
        Unit::Command(u) => u.name.clone(),
        Unit::Subagent(u) => u.name.clone(),
        Unit::Mcp(u) => u.name.clone(),
        _ => String::new(),
    }
}

/// Find an earlier `install_sha` in the scope lockfile for this
/// `(unit_type, source, agent)` tuple, if any.
fn prior_install_sha(
    scope: CoreScope,
    unit_type: UnitType,
    source: &Source,
    agent_id: &str,
) -> Option<rig_core::source::Sha256> {
    let lock = store::load_lockfile(scope).ok()?;
    let id = lock_id(unit_type, source);
    lock.entries
        .into_iter()
        .find(|e| e.id == id && e.agent.as_str() == agent_id && e.scope == scope)
        .map(|e| e.install_sha)
}

/// Detect drift against the current install and apply the chosen
/// resolution mode. Returns `Ok(Some(receipt))` on success,
/// `Ok(None)` when the write was skipped, or an error.
fn apply_with_drift_resolution(
    adapter: &dyn Adapter,
    unit: &Unit,
    unit_ref: &UnitRef,
    scope: CoreScope,
    prior_install_sha: Option<rig_core::source::Sha256>,
    on_drift: OnDrift,
) -> Result<Option<Receipt>> {
    let incoming_native = native_for(adapter, unit)?;

    // Compute current on-disk layout, if any. When `read_local` fails
    // (user broke the frontmatter, hand-edited a required field away,
    // etc.) we still want to know whether files exist so we snapshot
    // before overwriting — falling back to the list-reported paths.
    let current_native = match adapter.read_local(unit_ref, scope) {
        Ok(local) => Some(native_for(adapter, &local)?),
        Err(_) => {
            let listed = adapter.list(scope).unwrap_or_default();
            listed
                .iter()
                .find(|u| {
                    u.unit_ref.unit_type == unit_ref.unit_type && u.unit_ref.name == unit_ref.name
                })
                .and_then(|iu| {
                    // Build a synthetic NativeLayout from on-disk bytes so
                    // drift detection and snapshots still work.
                    let files: Vec<rig_core::converter::NativeFile> = iu
                        .paths
                        .iter()
                        .filter(|p| p.exists())
                        .filter_map(|p| {
                            let bytes = std::fs::read(p).ok()?;
                            let rel = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            Some(rig_core::converter::NativeFile {
                                relative_path: rel,
                                bytes,
                            })
                        })
                        .collect();
                    if files.is_empty() {
                        None
                    } else {
                        Some(NativeLayout { files })
                    }
                })
        }
    };

    // Clean shortcut: if prior install_sha == current on-disk hash AND
    // the incoming bytes equal the on-disk bytes, there's no write to do.
    if let (Some(cur), Some(prior)) = (&current_native, &prior_install_sha) {
        let cur_sha = hash_layout(cur);
        if cur_sha == *prior && hash_layout(&incoming_native) == cur_sha {
            println!(
                "  · {}/{} [{}] already up to date",
                type_slug(unit_ref.unit_type),
                unit_ref.name,
                adapter.agent(),
            );
            // No write; return a synthetic receipt reflecting the current state.
            return Ok(Some(Receipt {
                unit_ref: unit_ref.clone(),
                agent: adapter.agent(),
                scope,
                paths: Vec::new(),
                install_sha: cur_sha,
            }));
        }
    }

    // Detect drift relative to the prior install_sha (if known).
    let drift_state = match &prior_install_sha {
        Some(sha) => adapter
            .detect_drift(unit_ref, scope, sha.clone(), None)
            .map(|(s, _)| s)
            .unwrap_or(DriftState::Missing),
        None => {
            // No lockfile entry. If a file already exists, treat as LocalDrift.
            if current_native.is_some() {
                DriftState::LocalDrift
            } else {
                DriftState::Missing
            }
        }
    };

    // Clean: bytes match. Missing is only safe to write when the disk
    // really is empty — if `current_native` is Some (typically because
    // the user tampered with the file into an unparseable state), we
    // must treat it as LocalDrift so the user's bytes are snapshotted.
    if drift_state == DriftState::Clean
        || (drift_state == DriftState::Missing && current_native.is_none())
    {
        let r = adapter.install(unit, scope)?;
        return Ok(Some(r));
    }

    // Otherwise: local (or both) drift. Honour on_drift.
    match on_drift {
        OnDrift::Keep => {
            println!(
                "  skipped (local-drift) {}/{} [{}]",
                type_slug(unit_ref.unit_type),
                unit_ref.name,
                adapter.agent(),
            );
            Ok(None)
        }
        // `Overwrite` and `SnapshotThenOverwrite` are now equivalent —
        // we never silently destroy local edits (CLAUDE.md §critical).
        // The latter is retained as an alias for existing scripts.
        OnDrift::Overwrite | OnDrift::SnapshotThenOverwrite => {
            let (snap_dir, n_copied) =
                snapshot_current(current_native.as_ref(), adapter, unit_ref, scope)?;
            let r = adapter.install(unit, scope)?;
            if n_copied > 0 {
                println!("  backed up {} file(s) to {}", n_copied, snap_dir.display());
            }
            println!(
                "  overwrote (had local-drift) {}/{} [{}]",
                type_slug(r.unit_ref.unit_type),
                r.unit_ref.name,
                adapter.agent(),
            );
            Ok(Some(r))
        }
        OnDrift::Cancel => {
            bail!(
                "drift on {}/{} [{}] and --on-drift=cancel; aborting",
                type_slug(unit_ref.unit_type),
                unit_ref.name,
                adapter.agent(),
            );
        }
        OnDrift::DiffPerFile => {
            let current = current_native.as_ref();
            let proceed = diff_and_prompt(current, &incoming_native);
            if !proceed {
                println!(
                    "  skipped (diff) {}/{} [{}]",
                    type_slug(unit_ref.unit_type),
                    unit_ref.name,
                    adapter.agent(),
                );
                return Ok(None);
            }
            let r = adapter.install(unit, scope)?;
            Ok(Some(r))
        }
    }
}

/// Run the adapter's `to_native` path via installing into a throwaway
/// directory is overkill — use the local converter directly.
fn native_for(adapter: &dyn Adapter, unit: &Unit) -> Result<NativeLayout> {
    // Each adapter exposes `read_local` that returns a Unit, and
    // `install` that writes native bytes. We don't have `to_native` on
    // the Adapter trait publicly; re-derive via the Converter impls
    // keyed by unit type. Fall back to the specific Converter exports.
    // NOTE: both adapters share the same converters semantically for
    // the unit types supported by the CLI.
    let agent = adapter.agent();
    let native = match (agent.as_str(), unit) {
        ("claude", Unit::Skill(u)) => SkillConverter.to_native(u)?,
        ("claude", Unit::Rule(u)) => RuleConverter.to_native(u)?,
        ("claude", Unit::Command(u)) => CommandConverter.to_native(u)?,
        ("claude", Unit::Subagent(u)) => SubagentConverter.to_native(u)?,
        ("claude", Unit::Mcp(u)) => ClaudeMCPConverter.to_native(u)?,
        ("codex", Unit::Skill(u)) => rig_adapter_codex::SkillConverter.to_native(u)?,
        ("codex", Unit::Rule(u)) => rig_adapter_codex::RuleConverter.to_native(u)?,
        ("codex", Unit::Command(u)) => rig_adapter_codex::CommandConverter.to_native(u)?,
        ("codex", Unit::Subagent(u)) => rig_adapter_codex::SubagentConverter.to_native(u)?,
        ("codex", Unit::Mcp(u)) => rig_adapter_codex::MCPConverter.to_native(u)?,
        _ => bail!("unsupported (agent, unit) combination for native diffing"),
    };
    Ok(native)
}

fn hash_layout(l: &NativeLayout) -> rig_core::source::Sha256 {
    let mut bytes = Vec::new();
    for f in &l.files {
        bytes.extend_from_slice(f.relative_path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&f.bytes);
        bytes.push(0);
    }
    rig_core::source::Sha256::of(&bytes)
}

/// Format a UNIX timestamp (seconds) as an RFC3339-ish directory name
/// safe on all filesystems: `YYYY-MM-DDTHH-MM-SSZ` (colons replaced).
fn rfc3339_dirname(secs: u64) -> String {
    // Minimal RFC3339 without extra crates. Uses the epoch-to-UTC algo.
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as u32;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}Z",
        y, m, d, hour, minute, second
    )
}

/// Copy (not rename) every on-disk file belonging to `(unit_ref, agent,
/// scope)` into `<scope-dir>/.rig/snapshots/<rfc3339>/<agent>/<type>/<name>/`.
/// Returns the snapshot directory and number of files copied.
fn snapshot_current(
    _current: Option<&NativeLayout>,
    adapter: &dyn Adapter,
    unit_ref: &UnitRef,
    scope: CoreScope,
) -> Result<(PathBuf, usize)> {
    let listed = adapter.list(scope).unwrap_or_default();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ts_dir = rfc3339_dirname(ts);

    // Snapshots always live under the scope's `.rig/` dir (project for
    // Project/Local, home/.rig for Global).
    let base = store::scope_dir(scope)?
        .join("snapshots")
        .join(&ts_dir)
        .join(adapter.agent().as_str())
        .join(type_slug(unit_ref.unit_type))
        .join(&unit_ref.name);

    let mut copied = 0usize;
    if let Some(iu) = listed
        .iter()
        .find(|u| u.unit_ref.unit_type == unit_ref.unit_type && u.unit_ref.name == unit_ref.name)
    {
        // Find the common ancestor of all paths so we preserve relative
        // layout under the snapshot directory.
        let root = common_parent(&iu.paths);
        for p in &iu.paths {
            if !p.exists() {
                continue;
            }
            let rel = match &root {
                Some(r) => p.strip_prefix(r).unwrap_or(p).to_path_buf(),
                None => PathBuf::from(p.file_name().map(|n| n.to_os_string()).unwrap_or_default()),
            };
            let dst = base.join(&rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating snapshot dir {}", parent.display()))?;
            }
            let bytes = std::fs::read(p)
                .with_context(|| format!("reading {} for snapshot", p.display()))?;
            rig_fs::atomic_write(&dst, &bytes)
                .with_context(|| format!("snapshotting {} → {}", p.display(), dst.display()))?;
            copied += 1;
        }
    }
    Ok((base, copied))
}

fn common_parent(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut iter = paths.iter().filter_map(|p| p.parent().map(PathBuf::from));
    let first = iter.next()?;
    Some(iter.fold(first, |acc, p| {
        let mut out = PathBuf::new();
        for (a, b) in acc.components().zip(p.components()) {
            if a == b {
                out.push(a.as_os_str());
            } else {
                break;
            }
        }
        out
    }))
}

/// Print a unified diff per file and prompt Y/n. Returns true on
/// confirm. Non-TTY stdin defaults to false (skip).
fn diff_and_prompt(current: Option<&NativeLayout>, incoming: &NativeLayout) -> bool {
    use similar::{ChangeTag, TextDiff};

    let empty = NativeLayout { files: Vec::new() };
    let current = current.unwrap_or(&empty);
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in &current.files {
        names.insert(f.relative_path.clone());
    }
    for f in &incoming.files {
        names.insert(f.relative_path.clone());
    }
    for name in &names {
        let a = current
            .files
            .iter()
            .find(|f| &f.relative_path == name)
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
            .unwrap_or_default();
        let b = incoming
            .files
            .iter()
            .find(|f| &f.relative_path == name)
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
            .unwrap_or_default();
        if a == b {
            continue;
        }
        println!("--- {} (current)", name);
        println!("+++ {} (incoming)", name);
        let diff = TextDiff::from_lines(&a, &b);
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            print!("{sign}{}", change);
        }
    }

    // Prompt.
    use std::io::Write as _;
    print!("apply changes? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    match std::io::stdin().read_line(&mut buf) {
        Ok(0) => false,
        Ok(_) => matches!(buf.trim(), "y" | "Y" | "yes"),
        Err(_) => false,
    }
}

fn fetch_unit(
    source: &Source,
    as_type: Option<UnitType>,
    source_ref: &str,
) -> Result<(Unit, rig_core::source::Sha256)> {
    let fetched = rig_source::fetch(source).with_context(|| format!("fetching `{source_ref}`"))?;

    let unit_type = match (fetched.detected, as_type) {
        (_, Some(t)) => t,
        (Some(t), None) => t,
        (None, None) => bail!(
            "could not auto-detect unit type for `{source_ref}`; pass `--as <type>` (skill|rule|command|subagent)"
        ),
    };

    let unit = match unit_type {
        UnitType::Skill => Unit::Skill(
            SkillConverter
                .parse_native(&fetched.native)
                .with_context(|| format!("parsing skill from `{source_ref}`"))?,
        ),
        UnitType::Rule => Unit::Rule(
            RuleConverter
                .parse_native(&fetched.native)
                .with_context(|| format!("parsing rule from `{source_ref}`"))?,
        ),
        UnitType::Command => Unit::Command(
            CommandConverter
                .parse_native(&fetched.native)
                .with_context(|| format!("parsing command from `{source_ref}`"))?,
        ),
        UnitType::Subagent => Unit::Subagent(
            SubagentConverter
                .parse_native(&fetched.native)
                .with_context(|| format!("parsing subagent from `{source_ref}`"))?,
        ),
        UnitType::Mcp => Unit::Mcp(
            ClaudeMCPConverter
                .parse_native(&fetched.native)
                .with_context(|| format!("parsing mcp from `{source_ref}`"))?,
        ),
        other => bail!("unit type `{other:?}` not yet supported by the CLI"),
    };

    Ok((unit, fetched.source_sha))
}

fn add_to_manifest(
    scope: CoreScope,
    source: &Source,
    unit_type: UnitType,
    agents: &[CliAgent],
) -> Result<()> {
    let mut manifest = store::load_manifest(scope)?;
    for ag in agents {
        let id = rig_core::agent::AgentId::new(ag.id());
        if !manifest.agents.targets.contains(&id) {
            manifest.agents.targets.push(id);
        }
    }
    let bundle = manifest
        .bundles
        .entry(DEFAULT_BUNDLE.to_owned())
        .or_insert_with(Bundle::default);
    let vec = bundle_slot(bundle, unit_type);
    let s = source.to_string();
    if !vec.iter().any(|x| x == &s) {
        vec.push(s);
    }
    store::save_manifest(scope, &manifest)
}

fn bundle_slot(b: &mut Bundle, t: UnitType) -> &mut Vec<String> {
    match t {
        UnitType::Skill => &mut b.skills,
        UnitType::Mcp => &mut b.mcps,
        UnitType::Rule => &mut b.rules,
        UnitType::Hook => &mut b.hooks,
        UnitType::Command => &mut b.commands,
        UnitType::Subagent => &mut b.subagents,
        UnitType::Plugin => &mut b.plugins,
    }
}

fn upsert_lock(
    scope: CoreScope,
    source: &Source,
    source_sha: rig_core::source::Sha256,
    receipt: &Receipt,
) -> Result<()> {
    let mut lock = store::load_lockfile(scope)?;
    let id = lock_id(receipt.unit_ref.unit_type, source);
    lock.entries
        .retain(|e| !(e.id == id && e.agent == receipt.agent && e.scope == scope));
    let native_name = if receipt.unit_ref.unit_type == UnitType::Mcp {
        Some(receipt.unit_ref.name.clone())
    } else {
        None
    };
    lock.entries.push(LockEntry {
        id,
        unit_type: receipt.unit_ref.unit_type,
        source: source.clone(),
        source_sha,
        install_sha: receipt.install_sha.clone(),
        agent: receipt.agent.clone(),
        scope,
        path: receipt
            .paths
            .first()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("")),
        native_name,
        extra: Default::default(),
    });
    store::save_lockfile(scope, &lock)
}

fn lock_id(t: UnitType, source: &Source) -> String {
    format!("{}/{}", type_slug(t), source)
}

fn sync(scope: CoreScope, on_drift: OnDrift) -> Result<()> {
    let manifest = store::load_manifest(scope)?;
    if manifest.bundles.is_empty() {
        println!(
            "no bundles declared in {}",
            store::manifest_path(scope)?.display()
        );
        return Ok(());
    }

    let mut new_lock = Lockfile::new();
    let mut installed = 0;
    let mut skipped = 0;

    // Targets: honour `[agents].targets` if set, else default to claude.
    let targets: Vec<CliAgent> = if manifest.agents.targets.is_empty() {
        vec![CliAgent::Claude]
    } else {
        manifest
            .agents
            .targets
            .iter()
            .filter_map(|id| match id.as_str() {
                "claude" => Some(CliAgent::Claude),
                "codex" => Some(CliAgent::Codex),
                other => {
                    eprintln!("  ⚠  unknown agent `{other}` in manifest; skipped");
                    None
                }
            })
            .collect()
    };

    for (name, bundle) in &manifest.bundles {
        for (src, ty) in iter_bundle_entries(bundle) {
            let source = Source::parse(&src)
                .with_context(|| format!("parsing `{src}` in bundle `{name}`"))?;
            let (unit, source_sha) = fetch_unit(&source, Some(ty), &src)
                .with_context(|| format!("bundle `{name}`: fetching `{src}`"))?;
            for &ag in &targets {
                let adapter = ag.adapter();
                if !adapter.capabilities().contains(&ty) {
                    continue;
                }

                let uname = canonical_name(&unit);
                let unit_ref = UnitRef::new(ty, uname);
                let prior = prior_install_sha(scope, ty, &source, ag.id());

                let receipt = apply_with_drift_resolution(
                    &*adapter, &unit, &unit_ref, scope, prior, on_drift,
                )
                .with_context(|| {
                    format!(
                        "bundle `{name}`: installing `{src}` into {}",
                        adapter.agent()
                    )
                })?;

                let Some(receipt) = receipt else {
                    skipped += 1;
                    continue;
                };

                println!(
                    "  ✓ {}/{}  ({src}) → {}",
                    type_slug(ty),
                    receipt.unit_ref.name,
                    adapter.agent(),
                );
                let native_name = if ty == UnitType::Mcp {
                    Some(receipt.unit_ref.name.clone())
                } else {
                    None
                };
                new_lock.entries.push(LockEntry {
                    id: lock_id(ty, &source),
                    unit_type: ty,
                    source: source.clone(),
                    source_sha: source_sha.clone(),
                    install_sha: receipt.install_sha,
                    agent: receipt.agent,
                    scope,
                    path: receipt.paths.into_iter().next().unwrap_or_default(),
                    native_name,
                    extra: Default::default(),
                });
                installed += 1;
            }
        }
    }

    // Preserve any prior lock entries for units we skipped (so they
    // remain tracked). Merge: existing entries for (id, agent, scope)
    // that we did NOT overwrite.
    if let Ok(prev) = store::load_lockfile(scope) {
        for e in prev.entries {
            let already = new_lock
                .entries
                .iter()
                .any(|n| n.id == e.id && n.agent == e.agent && n.scope == e.scope);
            if !already {
                new_lock.entries.push(e);
            }
        }
    }

    store::save_lockfile(scope, &new_lock)?;
    println!(
        "synced {installed} unit(s){}; lockfile at {}",
        if skipped > 0 {
            format!(", {skipped} skipped (drift)")
        } else {
            String::new()
        },
        store::lockfile_path(scope)?.display()
    );
    Ok(())
}

fn iter_bundle_entries(b: &Bundle) -> Vec<(String, UnitType)> {
    let mut out = Vec::new();
    for s in &b.skills {
        out.push((s.clone(), UnitType::Skill));
    }
    for s in &b.rules {
        out.push((s.clone(), UnitType::Rule));
    }
    for s in &b.commands {
        out.push((s.clone(), UnitType::Command));
    }
    for s in &b.subagents {
        out.push((s.clone(), UnitType::Subagent));
    }
    // mcp / hook / plugin handled in later wedges.
    out
}

#[derive(Serialize)]
struct StatusEntry<'a> {
    agent: &'a str,
    unit_type: &'static str,
    name: String,
    scope: &'static str,
    state: &'static str,
    install_sha: String,
    current_sha: Option<String>,
    upstream_sha: Option<String>,
    path: String,
}

fn status(scope: CoreScope, json: bool) -> Result<()> {
    let lock = store::load_lockfile(scope)?;
    if lock.entries.is_empty() {
        if json {
            println!("[]");
        } else {
            println!(
                "no lockfile entries at {}",
                store::lockfile_path(scope)?.display()
            );
        }
        return Ok(());
    }
    let mut clean = 0;
    let mut checked = 0;
    let mut rows: Vec<StatusEntry<'_>> = Vec::new();
    for e in &lock.entries {
        if e.scope != scope {
            continue;
        }
        let adapter: Box<dyn Adapter> = match e.agent.as_str() {
            s if s == rig_adapter_claude::AGENT_ID => Box::new(ClaudeAdapter::new()),
            s if s == rig_adapter_codex::AGENT_ID => Box::new(CodexAdapter::new()),
            other => {
                eprintln!("  ⚠  unknown agent `{other}` in lockfile; skipped");
                continue;
            }
        };
        checked += 1;
        let name = if let Some(n) = &e.native_name {
            // MCP (and any future agent-entry-backed unit) keeps its
            // canonical name in `native_name` rather than the path.
            n.clone()
        } else if e.unit_type == UnitType::Skill {
            e.path
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            e.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let (state, shas) = adapter
            .detect_drift(
                &UnitRef::new(e.unit_type, &name),
                scope,
                e.install_sha.clone(),
                None,
            )
            .unwrap_or((
                DriftState::Missing,
                rig_core::drift::DriftShas {
                    install_time: e.install_sha.clone(),
                    current_disk: None,
                    upstream: None,
                },
            ));
        let marker = drift_slug(state);
        if state == DriftState::Clean {
            clean += 1;
        }
        if json {
            rows.push(StatusEntry {
                agent: e.agent.as_str(),
                unit_type: type_slug(e.unit_type),
                name: name.clone(),
                scope: scope_slug(scope),
                state: marker,
                install_sha: shas.install_time.to_string(),
                current_sha: shas.current_disk.map(|s| s.to_string()),
                upstream_sha: shas.upstream.map(|s| s.to_string()),
                path: e.path.display().to_string(),
            });
        } else {
            println!(
                "  {:<14} {}/{}  [{}]",
                marker,
                type_slug(e.unit_type),
                name,
                e.agent
            );
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{clean}/{checked} clean");
    }
    Ok(())
}

fn drift_slug(s: DriftState) -> &'static str {
    match s {
        DriftState::Clean => "clean",
        DriftState::LocalDrift => "local-drift",
        DriftState::UpstreamDrift => "upstream-drift",
        DriftState::BothDrift => "both-drift",
        DriftState::Orphan => "orphan",
        DriftState::Missing => "missing",
    }
}

#[derive(Serialize)]
struct ListEntry<'a> {
    agent: &'a str,
    unit_type: &'static str,
    name: String,
    scope: &'static str,
    paths: Vec<String>,
    linked: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    disabled: bool,
}

fn list(scope: CoreScope, json: bool) -> Result<()> {
    let all = collect_all(&[scope])?;
    let link_keys = link_key_set(&[scope]);
    if json {
        let rows: Vec<ListEntry<'_>> = all
            .iter()
            .map(|(agent, sc, u)| ListEntry {
                agent: agent.as_str(),
                unit_type: type_slug(u.unit_ref.unit_type),
                name: u.unit_ref.name.clone(),
                scope: scope_slug(*sc),
                paths: u.paths.iter().map(|p| p.display().to_string()).collect(),
                linked: link_keys.contains(&(
                    agent.as_str().to_owned(),
                    u.unit_ref.unit_type,
                    u.unit_ref.name.clone(),
                    *sc,
                )),
                disabled: u.disabled,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if all.is_empty() {
        println!("no units installed in any agent ({scope})");
        return Ok(());
    }
    for (agent, sc, u) in &all {
        let linked = link_keys.contains(&(
            agent.as_str().to_owned(),
            u.unit_ref.unit_type,
            u.unit_ref.name.clone(),
            *sc,
        ));
        println!(
            "{}/{} ({} file{}) [{}]{}{}",
            type_slug(u.unit_ref.unit_type),
            u.unit_ref.name,
            u.paths.len(),
            if u.paths.len() == 1 { "" } else { "s" },
            agent,
            if linked { " (linked)" } else { "" },
            if u.disabled { " [disabled]" } else { "" },
        );
    }
    Ok(())
}

/// Collect links.toml entries across the given scopes into a set of
/// `(agent_id, unit_type, name, scope)` tuples for O(1) lookup.
fn link_key_set(
    scopes: &[CoreScope],
) -> std::collections::HashSet<(String, UnitType, String, CoreScope)> {
    let mut out = std::collections::HashSet::new();
    for &sc in scopes {
        if let Ok(links) = store::load_links(sc) {
            for e in links.entries {
                out.insert((e.agent, e.unit_type, e.name, sc));
            }
        }
    }
    out
}

fn uninstall(target: &str, scope: CoreScope) -> Result<()> {
    let (ty_slug, name) = target
        .split_once('/')
        .with_context(|| format!("target must be `<type>/<name>`, got `{target}`"))?;
    let unit_type = parse_type(ty_slug)?;

    for adapter in [
        Box::new(ClaudeAdapter::new()) as Box<dyn Adapter>,
        Box::new(CodexAdapter::new()),
    ] {
        if !adapter.capabilities().contains(&unit_type) {
            continue;
        }
        adapter
            .uninstall(&UnitRef::new(unit_type, name), scope)
            .with_context(|| format!("uninstalling {target} from {} ({scope})", adapter.agent()))?;
        println!("removed {target} from {} ({scope})", adapter.agent());
    }

    if let Ok(mut lock) = store::load_lockfile(scope) {
        let before = lock.entries.len();
        lock.entries.retain(|e| {
            !(e.unit_type == unit_type
                && e.scope == scope
                && e.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|stem| stem == name))
        });
        if lock.entries.len() != before {
            store::save_lockfile(scope, &lock).ok();
        }
    }

    Ok(())
}

// ---------- New: link ----------

fn link(path: &Path, scope: CoreScope, agents: &[CliAgent], force: bool) -> Result<()> {
    #[cfg(not(unix))]
    {
        let _ = (path, scope, agents, force);
        bail!("`rig link` is Unix-only in M1");
    }
    #[cfg(unix)]
    {
        if !path.is_dir() {
            bail!("`{}` is not a directory", path.display());
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            bail!(
                "`{}` has no SKILL.md (only skills are linkable in M1)",
                path.display()
            );
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("invalid basename for `{}`", path.display()))?
            .to_owned();
        let abs_src = std::fs::canonicalize(path)
            .with_context(|| format!("canonicalising `{}`", path.display()))?;

        for &ag in agents {
            let target = link_target(ag, scope, &name)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            let exists = target.exists() || target.symlink_metadata().is_ok();
            if exists {
                if !force {
                    bail!(
                        "target `{}` already exists (use --force to overwrite)",
                        target.display()
                    );
                }
                // Remove existing (symlink or dir).
                let meta = target
                    .symlink_metadata()
                    .with_context(|| format!("stat {}", target.display()))?;
                if meta.file_type().is_dir() {
                    std::fs::remove_dir_all(&target)
                        .with_context(|| format!("removing {}", target.display()))?;
                } else {
                    std::fs::remove_file(&target)
                        .with_context(|| format!("removing {}", target.display()))?;
                }
            }
            std::os::unix::fs::symlink(&abs_src, &target)
                .with_context(|| format!("symlink {} → {}", target.display(), abs_src.display()))?;
            println!("linked {} skill/{} → {}", ag.id(), name, abs_src.display());

            upsert_link(
                scope,
                &store::LinkEntry {
                    agent: ag.id().to_owned(),
                    name: name.clone(),
                    unit_type: UnitType::Skill,
                    source: abs_src.clone(),
                },
            )?;
        }
        Ok(())
    }
}

fn upsert_link(scope: CoreScope, entry: &store::LinkEntry) -> Result<()> {
    let mut links = store::load_links(scope)?;
    links.entries.retain(|e| {
        !(e.agent == entry.agent && e.name == entry.name && e.unit_type == entry.unit_type)
    });
    links.entries.push(entry.clone());
    store::save_links(scope, &links)
}

fn unlink(target: &str, agents: Option<&[CliAgent]>, scope: CoreScope) -> Result<()> {
    let (ty_slug, name) = target
        .split_once('/')
        .with_context(|| format!("target must be `<type>/<name>`, got `{target}`"))?;
    let unit_type = parse_type(ty_slug)?;
    let agents_to_remove: Vec<CliAgent> = match agents {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => vec![CliAgent::Claude, CliAgent::Codex],
    };

    let mut links = store::load_links(scope)?;
    let mut any = false;

    for &ag in &agents_to_remove {
        // Remove the symlink itself (best-effort).
        if unit_type == UnitType::Skill {
            if let Ok(link_path) = link_target(ag, scope, name) {
                if link_path.symlink_metadata().is_ok() {
                    // symlink or dir
                    let ft = link_path.symlink_metadata().unwrap().file_type();
                    if ft.is_symlink() {
                        std::fs::remove_file(&link_path)
                            .with_context(|| format!("removing symlink {}", link_path.display()))?;
                        println!("unlinked {} {}/{}", ag.id(), ty_slug, name);
                        any = true;
                    }
                }
            }
        }
    }

    let before = links.entries.len();
    links.entries.retain(|e| {
        !(e.name == name
            && e.unit_type == unit_type
            && agents_to_remove.iter().any(|a| a.id() == e.agent))
    });
    if links.entries.len() != before {
        store::save_links(scope, &links)?;
        any = true;
    }

    if !any {
        println!("no link entry found for {target}");
    }
    Ok(())
}

fn link_target(ag: CliAgent, scope: CoreScope, name: &str) -> Result<PathBuf> {
    let root: PathBuf = match scope {
        CoreScope::Global => rig_fs::home_dir().context("discovering home dir")?,
        // `Local` is Claude-only for MCP — `rig link` is a skill
        // symlink helper, so both Project and Local point at the CWD.
        CoreScope::Project | CoreScope::Local => PathBuf::from("."),
    };
    let sub = match ag {
        CliAgent::Claude => [".claude", "skills"],
        CliAgent::Codex => [".codex", "skills"],
    };
    Ok(root.join(sub[0]).join(sub[1]).join(name))
}

// ---------- New: init-skill ----------

fn init_skill(name: &str, in_dir: Option<&Path>) -> Result<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        bail!("invalid skill name `{name}`");
    }
    let parent = in_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = parent.join(name);
    if dir.exists() {
        bail!("`{}` already exists", dir.display());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let skill_md = dir.join("SKILL.md");
    let title = title_case(name);
    let body = format!(
        "---\nname: {name}\ndescription: One-line description of what this skill does. The agent reads this when deciding to invoke.\n---\n\n# {title}\n\nDetailed instructions for the agent go here. Markdown.\n\n## When to use\n\nDescribe trigger conditions.\n\n## Examples\n\n- Example 1\n- Example 2\n"
    );
    rig_fs::atomic_write(&skill_md, body.as_bytes())
        .with_context(|| format!("writing {}", skill_md.display()))?;
    println!("created {}", skill_md.display());
    Ok(())
}

fn init_mcp(name: &str, in_dir: Option<&Path>, transport: CliMcpTransport) -> Result<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        bail!("invalid mcp name `{name}`");
    }
    let parent = in_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = parent.join(name);
    if dir.exists() {
        bail!("`{}` already exists", dir.display());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let mcp_toml = dir.join("mcp.toml");
    let body = match transport {
        CliMcpTransport::Stdio => format!(
            "# MCP server config consumed by Rig. See docs/MCP-SUPPORT.md.\n\
             schema = \"rig/v1\"\n\
             kind = \"mcp\"\n\
             name = \"{name}\"\n\
             description = \"One-line description for when the agent should use this server.\"\n\
             \n\
             [transport]\n\
             kind = \"stdio\"\n\
             command = \"npx\"\n\
             args = [\"-y\", \"@example/mcp\"]\n\
             \n\
             # Declare env var names the server reads. Values live in the\n\
             # shell env / agent-native secret store — Rig never embeds them.\n\
             # env = [\"API_KEY\"]\n"
        ),
        CliMcpTransport::Http => format!(
            "# MCP server config consumed by Rig. See docs/MCP-SUPPORT.md.\n\
             schema = \"rig/v1\"\n\
             kind = \"mcp\"\n\
             name = \"{name}\"\n\
             description = \"One-line description for when the agent should use this server.\"\n\
             \n\
             [transport]\n\
             kind = \"http\"\n\
             url = \"https://example.com/mcp\"\n\
             \n\
             # [transport.headers]\n\
             # Authorization = \"Bearer ${{API_TOKEN}}\"\n"
        ),
        CliMcpTransport::Sse => format!(
            "# MCP server config consumed by Rig. See docs/MCP-SUPPORT.md.\n\
             schema = \"rig/v1\"\n\
             kind = \"mcp\"\n\
             name = \"{name}\"\n\
             description = \"One-line description for when the agent should use this server.\"\n\
             \n\
             [transport]\n\
             kind = \"sse\"\n\
             url = \"https://example.com/sse\"\n\
             \n\
             # [transport.headers]\n\
             # Authorization = \"Bearer ${{API_TOKEN}}\"\n"
        ),
    };
    rig_fs::atomic_write(&mcp_toml, body.as_bytes())
        .with_context(|| format!("writing {}", mcp_toml.display()))?;
    println!("created {}", mcp_toml.display());
    Ok(())
}

#[derive(Serialize)]
struct InspectFile {
    path: String,
    size: usize,
    sha256: String,
}

#[derive(Serialize)]
struct InspectJson<'a> {
    source: String,
    source_sha: String,
    detected_type: Option<&'a str>,
    files: Vec<InspectFile>,
    name: Option<String>,
    description: Option<String>,
}

fn inspect(
    source_ref: &str,
    json: bool,
    raw: Option<&str>,
    as_type: Option<UnitType>,
) -> Result<()> {
    let source =
        Source::parse(source_ref).with_context(|| format!("parsing source `{source_ref}`"))?;
    let fetched = rig_source::fetch(&source).with_context(|| format!("fetching `{source_ref}`"))?;

    // --raw short-circuits everything else and dumps exact bytes.
    if let Some(target) = raw {
        let file = fetched
            .native
            .files
            .iter()
            .find(|f| f.relative_path == target)
            .ok_or_else(|| anyhow::anyhow!("no such file in source: {target}"))?;
        use std::io::Write;
        std::io::stdout()
            .write_all(&file.bytes)
            .context("writing bytes to stdout")?;
        return Ok(());
    }

    // Try to parse the unit so we can surface name + description.
    let (name, description) = match as_type.or(fetched.detected) {
        Some(UnitType::Skill) => SkillConverter
            .parse_native(&fetched.native)
            .map(|s| (Some(s.name), Some(s.description)))
            .unwrap_or((None, None)),
        Some(UnitType::Rule) => RuleConverter
            .parse_native(&fetched.native)
            .map(|r| (Some(r.name), r.description))
            .unwrap_or((None, None)),
        Some(UnitType::Command) => CommandConverter
            .parse_native(&fetched.native)
            .map(|c| (Some(c.name), c.description))
            .unwrap_or((None, None)),
        Some(UnitType::Subagent) => SubagentConverter
            .parse_native(&fetched.native)
            .map(|s| (Some(s.name), Some(s.description)))
            .unwrap_or((None, None)),
        Some(UnitType::Mcp) => ClaudeMCPConverter
            .parse_native(&fetched.native)
            .map(|m| (Some(m.name), m.description))
            .unwrap_or((None, None)),
        _ => (None, None),
    };

    if json {
        let out = InspectJson {
            source: source.to_string(),
            source_sha: fetched.source_sha.to_string(),
            detected_type: fetched.detected.map(type_slug),
            files: fetched
                .native
                .files
                .iter()
                .map(|f| InspectFile {
                    path: f.relative_path.clone(),
                    size: f.bytes.len(),
                    sha256: rig_core::source::Sha256::of(&f.bytes).to_string(),
                })
                .collect(),
            name,
            description,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!("source: {}", source);
    println!("source_sha: {}", fetched.source_sha);
    match fetched.detected {
        Some(t) => println!("type: {}", type_slug(t)),
        None => println!("type: (not auto-detected; pass --as <type>)"),
    }
    if let Some(n) = &name {
        println!("name: {}", n);
    }
    if let Some(d) = &description {
        println!("description: {}", d);
    }
    println!("\nfiles:");
    for f in &fetched.native.files {
        let sha = rig_core::source::Sha256::of(&f.bytes).to_string();
        let short: String = sha.chars().take(8).collect();
        println!("  {} ({} B, sha {})", f.relative_path, f.bytes.len(), short);
    }

    // Body preview for the canonical entry file if present.
    let preview_target = fetched.native.files.iter().find(|f| {
        matches!(
            f.relative_path.as_str(),
            "SKILL.md" | "AGENTS.md" | "mcp.toml"
        )
    });
    if let Some(f) = preview_target {
        let text = String::from_utf8_lossy(&f.bytes);
        println!("\n{} preview (first 20 lines):", f.relative_path);
        for line in text.lines().take(20) {
            println!("  {}", line);
        }
    }
    Ok(())
}

fn title_case(s: &str) -> String {
    s.split(['-', '_', ' '])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- New: search ----------

#[derive(Serialize)]
struct SearchRow<'a> {
    agent: &'a str,
    unit_type: &'static str,
    name: String,
    scope: &'static str,
    paths: Vec<String>,
    linked: bool,
    matched_in: &'static str,
}

/// Where in a unit a substring search matched. Priority (highest wins):
/// name > description > body.
#[derive(Copy, Clone)]
enum MatchSite {
    Name,
    Description,
    Body,
}

impl MatchSite {
    fn slug(self) -> &'static str {
        match self {
            MatchSite::Name => "name",
            MatchSite::Description => "description",
            MatchSite::Body => "body",
        }
    }
}

/// Extract (description, body) from a canonical Unit. Either may be
/// empty when the unit type does not carry that field.
fn unit_description_body(u: &Unit) -> (String, String) {
    match u {
        Unit::Skill(s) => (s.description.clone(), s.body.clone()),
        Unit::Rule(r) => (r.description.clone().unwrap_or_default(), r.body.clone()),
        Unit::Command(c) => (c.description.clone().unwrap_or_default(), c.body.clone()),
        Unit::Subagent(s) => (s.description.clone(), s.body.clone()),
        Unit::Mcp(m) => (m.description.clone().unwrap_or_default(), String::new()),
        _ => (String::new(), String::new()),
    }
}

/// Try to return 3 lines of context around the first match of `q` in
/// `text`. Returns None when there is no hit.
fn match_snippet(text: &str, q: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let idx = lower.find(&q.to_lowercase())?;
    // Find line number of idx.
    let before = &text[..idx];
    let line_no = before.bytes().filter(|b| *b == b'\n').count();
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let lo = line_no.saturating_sub(1);
    let hi = (line_no + 2).min(lines.len());
    Some(
        lines[lo..hi]
            .iter()
            .map(|l| format!("    {}", l))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn search(query: &str, scope: CliScopeAll, json: bool, full: bool) -> Result<()> {
    let scopes = scope.scopes();
    let all = collect_all(&scopes)?;
    let link_keys = link_key_set(&scopes);
    let q_lower = query.to_lowercase();
    let scan_body = full && query.len() >= 3;

    // Collect matches + site + optional snippet.
    let mut matches: Vec<(
        rig_core::agent::AgentId,
        CoreScope,
        InstalledUnit,
        MatchSite,
        Option<String>,
    )> = Vec::new();
    for (agent, sc, u) in all.into_iter() {
        // Cheap checks first.
        if u.unit_ref.name.to_lowercase().contains(&q_lower)
            || type_slug(u.unit_ref.unit_type).contains(&q_lower)
        {
            matches.push((agent, sc, u, MatchSite::Name, None));
            continue;
        }

        // Description match — read_local to pull frontmatter. Body too
        // when --full.
        let adapter: Box<dyn Adapter> = match agent.as_str() {
            s if s == rig_adapter_claude::AGENT_ID => Box::new(ClaudeAdapter::new()),
            s if s == rig_adapter_codex::AGENT_ID => Box::new(CodexAdapter::new()),
            _ => continue,
        };
        let local = match adapter.read_local(&u.unit_ref, sc) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let (desc, body) = unit_description_body(&local);
        if desc.to_lowercase().contains(&q_lower) {
            let snip = match_snippet(&desc, query);
            matches.push((agent, sc, u, MatchSite::Description, snip));
            continue;
        }
        if scan_body && body.to_lowercase().contains(&q_lower) {
            let snip = match_snippet(&body, query);
            matches.push((agent, sc, u, MatchSite::Body, snip));
        }
    }

    if json {
        let rows: Vec<SearchRow<'_>> = matches
            .iter()
            .map(|(agent, sc, u, site, _)| SearchRow {
                agent: agent.as_str(),
                unit_type: type_slug(u.unit_ref.unit_type),
                name: u.unit_ref.name.clone(),
                scope: scope_slug(*sc),
                paths: u.paths.iter().map(|p| p.display().to_string()).collect(),
                linked: link_keys.contains(&(
                    agent.as_str().to_owned(),
                    u.unit_ref.unit_type,
                    u.unit_ref.name.clone(),
                    *sc,
                )),
                matched_in: site.slug(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if matches.is_empty() {
        println!("no matches");
        return Ok(());
    }
    for (agent, sc, u, site, snip) in &matches {
        let path = u
            .paths
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let linked = link_keys.contains(&(
            agent.as_str().to_owned(),
            u.unit_ref.unit_type,
            u.unit_ref.name.clone(),
            *sc,
        ));
        println!(
            "{}/{}  [{}]  ({})  {}{}  <{}>",
            type_slug(u.unit_ref.unit_type),
            u.unit_ref.name,
            agent,
            scope_slug(*sc),
            path,
            if linked { "  (linked)" } else { "" },
            site.slug(),
        );
        if let Some(s) = snip {
            println!("{}", s);
        }
    }
    Ok(())
}

/// Legacy helper, retained for callers/tests that only want name+type.
#[allow(dead_code)]
fn matches_query(q: &str, ty: UnitType, name: &str) -> bool {
    let q = q.to_lowercase();
    name.to_lowercase().contains(&q) || type_slug(ty).contains(&q)
}

// ---------- New: stats ----------

#[derive(Serialize)]
struct StatsTypeBucket {
    count: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct StatsAgentBlock<'a> {
    agent: &'a str,
    scope: &'static str,
    by_type: std::collections::BTreeMap<&'static str, StatsTypeBucket>,
    total_count: u64,
    total_bytes: u64,
}

#[derive(Serialize)]
struct StatsOutput<'a> {
    agents: Vec<StatsAgentBlock<'a>>,
    grand_total: StatsTypeBucket,
}

fn stats(scope: CliScopeAll, json: bool) -> Result<()> {
    use std::collections::BTreeMap;

    let scopes = scope.scopes();
    let mut blocks: Vec<StatsAgentBlock<'_>> = Vec::new();
    let mut grand = StatsTypeBucket { count: 0, bytes: 0 };

    for ag in [CliAgent::Claude, CliAgent::Codex] {
        let adapter = ag.adapter();
        for &sc in &scopes {
            let units = adapter.list(sc).unwrap_or_default();
            let mut by_type: BTreeMap<&'static str, StatsTypeBucket> = BTreeMap::new();
            let mut total_count = 0u64;
            let mut total_bytes = 0u64;
            for u in &units {
                let bytes: u64 = u
                    .paths
                    .iter()
                    .filter_map(|p| std::fs::metadata(p).ok())
                    .map(|m| m.len())
                    .sum();
                let slot = by_type
                    .entry(type_slug(u.unit_ref.unit_type))
                    .or_insert(StatsTypeBucket { count: 0, bytes: 0 });
                slot.count += 1;
                slot.bytes += bytes;
                total_count += 1;
                total_bytes += bytes;
            }
            grand.count += total_count;
            grand.bytes += total_bytes;
            blocks.push(StatsAgentBlock {
                agent: ag.id(),
                scope: scope_slug(sc),
                by_type,
                total_count,
                total_bytes,
            });
        }
    }

    if json {
        let out = StatsOutput {
            agents: blocks,
            grand_total: grand,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    for b in &blocks {
        println!("{} ({})", b.agent, b.scope);
        if b.total_count == 0 {
            println!("  (empty)");
            println!();
            continue;
        }
        for (ty, bucket) in &b.by_type {
            println!(
                "  {:<9} {:>3}  ({})",
                format!("{ty}:"),
                bucket.count,
                human_bytes(bucket.bytes)
            );
        }
        println!(
            "  {:<9} {:>3}  ({})",
            "total:",
            b.total_count,
            human_bytes(b.total_bytes)
        );
        println!();
    }
    println!(
        "Grand total: {} units, {}",
        grand.count,
        human_bytes(grand.bytes)
    );
    Ok(())
}

fn human_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if n < KB {
        format!("{n} B")
    } else if n < MB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else if n < GB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else {
        format!("{:.1} GB", n as f64 / GB as f64)
    }
}

// ---------- New: doctor ----------

fn doctor(fix: bool) -> Result<()> {
    use std::collections::HashMap;

    let all = collect_all(&[CoreScope::Global, CoreScope::Project, CoreScope::Local])?;

    // Duplicates: same (unit_type, name) seen on 2+ agents (any scope).
    type DupEntry = (String, CoreScope, PathBuf);
    let mut by_key: HashMap<(UnitType, String), Vec<DupEntry>> = HashMap::new();
    for (agent, sc, u) in &all {
        let first = u.paths.first().cloned().unwrap_or_default();
        by_key
            .entry((u.unit_ref.unit_type, u.unit_ref.name.clone()))
            .or_default()
            .push((agent.as_str().to_owned(), *sc, first));
    }

    let mut dup_count = 0;
    for ((ty, name), entries) in &by_key {
        let distinct_agents: std::collections::BTreeSet<_> =
            entries.iter().map(|(a, _, _)| a.as_str()).collect();
        if distinct_agents.len() >= 2 {
            dup_count += 1;
            println!("duplicate: {}/{}", type_slug(*ty), name);
            for (agent, sc, path) in entries {
                println!("  - {:<6} ({}): {}", agent, scope_slug(*sc), path.display());
            }
        }
    }

    // Broken symlinks: any path in InstalledUnit.paths that is a symlink
    // to a non-existent target.
    let mut broken_count = 0;
    for (_agent, _sc, u) in &all {
        for p in &u.paths {
            if let Ok(meta) = p.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    match std::fs::read_link(p) {
                        Ok(target) => {
                            let absolute = if target.is_absolute() {
                                target.clone()
                            } else {
                                p.parent()
                                    .map(|par| par.join(&target))
                                    .unwrap_or_else(|| target.clone())
                            };
                            if !absolute.exists() {
                                broken_count += 1;
                                println!("broken symlink: {} → {}", p.display(), target.display());
                            }
                        }
                        Err(_) => {
                            broken_count += 1;
                            println!("broken symlink: {} → ?", p.display());
                        }
                    }
                }
            }
        }
    }

    // Broken link entries: source path no longer exists, or the
    // symlink itself is missing from disk.
    let mut broken_link_count = 0;
    for sc in [CoreScope::Global, CoreScope::Project] {
        let Ok(links) = store::load_links(sc) else {
            continue;
        };
        for e in &links.entries {
            let ag = match e.agent.as_str() {
                s if s == rig_adapter_claude::AGENT_ID => CliAgent::Claude,
                s if s == rig_adapter_codex::AGENT_ID => CliAgent::Codex,
                _ => continue,
            };
            if !e.source.exists() {
                broken_link_count += 1;
                println!(
                    "broken link source: [{}] {}/{} → {} (source missing)",
                    e.agent,
                    type_slug(e.unit_type),
                    e.name,
                    e.source.display(),
                );
            }
            if e.unit_type == UnitType::Skill {
                if let Ok(lp) = link_target(ag, sc, &e.name) {
                    if lp.symlink_metadata().is_err() {
                        broken_link_count += 1;
                        println!(
                            "broken link: [{}] {}/{} missing at {}",
                            e.agent,
                            type_slug(e.unit_type),
                            e.name,
                            lp.display(),
                        );
                    }
                }
            }
        }
    }

    // ----- Mv reconciliation (spec ENABLE-DISABLE-MV.md §8 / §12) -----
    //
    // Build a map of `(agent, unit_type, name) -> set<scope>` for both
    // (a) what the adapters see on disk, and (b) what the lockfiles
    // claim. Any mismatch is either split-state or stale-lock-entry.
    type Triple = (String, UnitType, String);
    let mut disk_scopes: HashMap<Triple, std::collections::HashSet<CoreScope>> = HashMap::new();
    for (agent, sc, u) in &all {
        disk_scopes
            .entry((
                agent.as_str().to_owned(),
                u.unit_ref.unit_type,
                u.unit_ref.name.clone(),
            ))
            .or_default()
            .insert(*sc);
    }

    // Collect lockfile claims per scope, keyed by (agent, type, name).
    let mut lock_scopes: HashMap<Triple, std::collections::HashSet<CoreScope>> = HashMap::new();
    for sc in [CoreScope::Global, CoreScope::Project, CoreScope::Local] {
        let Ok(lock) = store::load_lockfile(sc) else {
            continue;
        };
        for e in &lock.entries {
            if e.scope != sc {
                continue;
            }
            let name = lock_entry_name(e);
            lock_scopes
                .entry((e.agent.as_str().to_owned(), e.unit_type, name))
                .or_default()
                .insert(sc);
        }
    }

    let mut split_count = 0;
    let mut stale_count = 0;
    let mut mv_header_printed = false;

    // Walk the union of keys to find all inconsistencies.
    let mut all_keys: std::collections::BTreeSet<Triple> = std::collections::BTreeSet::new();
    for k in disk_scopes.keys() {
        all_keys.insert(k.clone());
    }
    for k in lock_scopes.keys() {
        all_keys.insert(k.clone());
    }

    for key in &all_keys {
        let disk = disk_scopes.get(key).cloned().unwrap_or_default();
        let lock = lock_scopes.get(key).cloned().unwrap_or_default();
        if disk == lock {
            continue;
        }
        // Split-state: disk has scopes that lock doesn't.
        let only_disk: Vec<_> = disk.difference(&lock).copied().collect();
        // Stale lock: lock has scopes that disk doesn't.
        let only_lock: Vec<_> = lock.difference(&disk).copied().collect();

        if !only_disk.is_empty() {
            if !mv_header_printed {
                println!("Mv reconciliation:");
                mv_header_printed = true;
            }
            split_count += 1;
            println!(
                "  split-state: [{}] {}/{} present on disk in {} but lockfile only covers {}",
                key.0,
                type_slug(key.1),
                key.2,
                format_scope_set(&only_disk),
                if lock.is_empty() {
                    "<none>".to_owned()
                } else {
                    format_scope_set(&lock.iter().copied().collect::<Vec<_>>())
                },
            );
            println!(
                "    fix: re-run `rig mv {}/{} --to <scope>` or manually remove from the stale scope",
                type_slug(key.1),
                key.2,
            );
        }

        if !only_lock.is_empty() {
            if !mv_header_printed {
                println!("Mv reconciliation:");
                mv_header_printed = true;
            }
            stale_count += 1;
            println!(
                "  stale-lock-entry: [{}] {}/{} lockfile in {} but not installed there",
                key.0,
                type_slug(key.1),
                key.2,
                format_scope_set(&only_lock),
            );
            if fix {
                for sc in &only_lock {
                    let Ok(mut l) = store::load_lockfile(*sc) else {
                        continue;
                    };
                    let before = l.entries.len();
                    l.entries.retain(|e| {
                        !(e.agent.as_str() == key.0
                            && e.unit_type == key.1
                            && lock_entry_name(e) == key.2
                            && e.scope == *sc)
                    });
                    if l.entries.len() != before {
                        store::save_lockfile(*sc, &l).ok();
                        println!("    fixed: dropped stale entry from {} lockfile", sc);
                    }
                }
            } else {
                println!("    fix: re-run `rig doctor --fix` to drop the stale entry");
            }
        }
    }

    if dup_count == 0
        && broken_count == 0
        && broken_link_count == 0
        && split_count == 0
        && stale_count == 0
    {
        println!("all clean");
    } else {
        println!(
            "{dup_count} duplicates, {broken_count} broken symlinks, {broken_link_count} broken links, {split_count} split-state, {stale_count} stale lock entries"
        );
    }
    Ok(())
}

/// Best-effort extraction of the canonical unit name from a LockEntry.
/// Prefers `native_name` (set for MCP), falls back to the path stem,
/// then to the trailing segment of `id`.
fn lock_entry_name(e: &LockEntry) -> String {
    if let Some(n) = &e.native_name {
        return n.clone();
    }
    if let Some(stem) = e.path.file_stem().and_then(|s| s.to_str()) {
        // Skill installs use a directory-per-skill layout, so the
        // file stem is often `SKILL`. In that case prefer the
        // parent directory name.
        if stem == "SKILL" {
            if let Some(parent) = e.path.parent().and_then(|p| p.file_name()) {
                if let Some(s) = parent.to_str() {
                    return s.to_owned();
                }
            }
        }
        return stem.to_owned();
    }
    e.id.rsplit_once('/')
        .map(|(_, n)| n.to_owned())
        .unwrap_or_else(|| e.id.clone())
}

fn format_scope_set(scopes: &[CoreScope]) -> String {
    let mut v: Vec<&'static str> = scopes.iter().copied().map(scope_slug).collect();
    v.sort_unstable();
    v.dedup();
    v.join("+")
}

// ---------- Shared helpers ----------

/// List every installed unit across both adapters and the given scopes.
///
/// Includes `rig link` entries from `links.toml` so they appear in
/// `list` / `search` / `stats` even when the adapter's native
/// `list()` misses them (e.g. symlinked dirs on platforms where
/// `file_type()` reports `Symlink` rather than `Dir`).
fn collect_all(
    scopes: &[CoreScope],
) -> Result<Vec<(rig_core::agent::AgentId, CoreScope, InstalledUnit)>> {
    // Pre-compute the set of `(agent, scope, mcp_name)` tuples that
    // Rig's lockfile claims ownership of. Used to filter foreign MCP
    // entries added directly via `claude mcp add` from `rig list`
    // output — spec MCP-SUPPORT.md §6 (deferred from Wedge A).
    let mut rig_mcp: std::collections::HashSet<(String, CoreScope, String)> =
        std::collections::HashSet::new();
    for &sc in scopes {
        if let Ok(lock) = store::load_lockfile(sc) {
            for e in &lock.entries {
                if e.unit_type == UnitType::Mcp {
                    let native_name = e.native_name.clone().unwrap_or_else(|| {
                        // Fall back to parsing the lockfile id.
                        e.id.rsplit_once('/')
                            .map(|(_, n)| n.to_owned())
                            .unwrap_or_default()
                    });
                    rig_mcp.insert((e.agent.as_str().to_owned(), e.scope, native_name));
                }
            }
        }
    }

    let mut out: Vec<(rig_core::agent::AgentId, CoreScope, InstalledUnit)> = Vec::new();
    for adapter in [
        Box::new(ClaudeAdapter::new()) as Box<dyn Adapter>,
        Box::new(CodexAdapter::new()),
    ] {
        for &sc in scopes {
            let units = match adapter.list(sc) {
                Ok(v) => v,
                Err(_) => continue,
            };
            for u in units {
                // Hide foreign MCP entries — anything the adapter
                // surfaces for UnitType::Mcp that is NOT in Rig's
                // lockfile.
                if u.unit_ref.unit_type == UnitType::Mcp {
                    let key = (
                        adapter.agent().as_str().to_owned(),
                        sc,
                        u.unit_ref.name.clone(),
                    );
                    if !rig_mcp.contains(&key) {
                        continue;
                    }
                }
                out.push((adapter.agent(), sc, u));
            }
        }
    }

    // Merge link entries not already in the native list.
    for &sc in scopes {
        let Ok(links) = store::load_links(sc) else {
            continue;
        };
        for e in links.entries {
            let ag = match e.agent.as_str() {
                s if s == rig_adapter_claude::AGENT_ID => CliAgent::Claude,
                s if s == rig_adapter_codex::AGENT_ID => CliAgent::Codex,
                _ => continue,
            };
            let agent_id = rig_core::agent::AgentId::new(ag.id());
            let already = out.iter().any(|(a, ss, u)| {
                a == &agent_id
                    && *ss == sc
                    && u.unit_ref.unit_type == e.unit_type
                    && u.unit_ref.name == e.name
            });
            if already {
                continue;
            }
            let link_path = link_target(ag, sc, &e.name).ok();
            let paths = link_path.into_iter().collect();
            out.push((
                agent_id,
                sc,
                InstalledUnit {
                    unit_ref: UnitRef::new(e.unit_type, e.name),
                    scope: sc,
                    paths,
                    disabled: false,
                },
            ));
        }
    }

    Ok(out)
}

/// Resolve the scope for a toggle target. If `--scope` was given, use
/// that. Otherwise look for the unit across global/project/local and
/// pick the one scope where it's installed. Returns exit-code 22 if
/// installed in more than one, 20 if none.
fn resolve_toggle_scope(
    unit_type: UnitType,
    name: &str,
    scope: Option<CliScope>,
) -> Result<CoreScope> {
    if let Some(s) = scope {
        return Ok(s.into());
    }
    let all = collect_all(&[CoreScope::Global, CoreScope::Project, CoreScope::Local])?;
    let matches: std::collections::HashSet<CoreScope> = all
        .iter()
        .filter(|(_, _, u)| u.unit_ref.unit_type == unit_type && u.unit_ref.name == name)
        .map(|(_, s, _)| *s)
        .collect();
    if matches.is_empty() {
        std::process::exit(20);
    }
    if matches.len() > 1 {
        eprintln!(
            "ambiguous scope: `{}/{}` is installed in {:?}; pass --scope",
            type_slug(unit_type),
            name,
            matches.iter().map(|s| scope_slug(*s)).collect::<Vec<_>>(),
        );
        std::process::exit(22);
    }
    Ok(*matches.iter().next().unwrap())
}

fn toggle(
    target: &str,
    agents: Option<&[CliAgent]>,
    scope: Option<CliScope>,
    enabled: bool,
) -> Result<()> {
    let (ty_slug, name) = target
        .split_once('/')
        .with_context(|| format!("target must be `<type>/<name>`, got `{target}`"))?;
    let unit_type = parse_type(ty_slug)?;

    // Guard against unsupported-in-M1 types up front (spec §7 exit 23).
    if matches!(unit_type, UnitType::Hook | UnitType::Plugin) {
        eprintln!(
            "{}/{} unit type does not support toggle (tracked as open question O1 in docs/ENABLE-DISABLE-MV.md)",
            ty_slug, name,
        );
        std::process::exit(23);
    }

    let scope = resolve_toggle_scope(unit_type, name, scope)?;

    // Figure out which agents to touch.
    let target_agents: Vec<CliAgent> = if let Some(a) = agents {
        a.to_vec()
    } else {
        // Query collect_all to see which agents actually have it.
        let all = collect_all(&[scope])?;
        let mut out: Vec<CliAgent> = Vec::new();
        for (agent_id, _, u) in &all {
            if u.unit_ref.unit_type != unit_type || u.unit_ref.name != name {
                continue;
            }
            let ag = match agent_id.as_str() {
                s if s == rig_adapter_claude::AGENT_ID => CliAgent::Claude,
                s if s == rig_adapter_codex::AGENT_ID => CliAgent::Codex,
                _ => continue,
            };
            if !out.contains(&ag) {
                out.push(ag);
            }
        }
        if out.is_empty() {
            std::process::exit(20);
        }
        out
    };

    let unit_ref = UnitRef::new(unit_type, name.to_owned());
    let verb = if enabled { "enabled" } else { "disabled" };
    let mut io_failure = false;
    let mut target_collision = false;
    let mut unsupported = false;

    println!("{verb} {target}");
    for ag in target_agents {
        let adapter = ag.adapter();
        match adapter.set_enabled(&unit_ref, scope, enabled) {
            Ok(()) => {
                println!("  {}  {scope}", ag.id());
            }
            Err(rig_core::adapter::AdapterError::UnsupportedOp(_))
            | Err(rig_core::adapter::AdapterError::Unsupported(_)) => {
                eprintln!("  {}  {scope}  [unsupported]", ag.id());
                unsupported = true;
            }
            Err(rig_core::adapter::AdapterError::TargetCollision { path }) => {
                eprintln!("  {}  {scope}  [collision: {path}]", ag.id());
                target_collision = true;
            }
            Err(rig_core::adapter::AdapterError::NotFound(_, _)) => {
                eprintln!("  {}  {scope}  [not installed]", ag.id());
                io_failure = true;
            }
            Err(e) => {
                eprintln!("  {}  {scope}  [error: {e}]", ag.id());
                io_failure = true;
            }
        }
    }

    if target_collision {
        std::process::exit(21);
    }
    if unsupported {
        std::process::exit(23);
    }
    if io_failure {
        std::process::exit(24);
    }
    Ok(())
}

/// Move an installed unit between scopes. Ordered best-effort per
/// `docs/ENABLE-DISABLE-MV.md` §8 — no two-phase commit. Crash windows
/// are surfaced by `rig doctor`.
fn mv(target: &str, to: CoreScope, agents: Option<&[CliAgent]>) -> Result<()> {
    let (ty_slug, name) = target
        .split_once('/')
        .with_context(|| format!("target must be `<type>/<name>`, got `{target}`"))?;
    let unit_type = parse_type(ty_slug)?;

    // Find every (agent, scope) the unit is installed in. Restrict to
    // agents the user specified, if any.
    let all = collect_all(&[CoreScope::Global, CoreScope::Project, CoreScope::Local])?;
    let filter_agents: Option<Vec<CliAgent>> = agents.map(<[CliAgent]>::to_vec);

    // For each agent that has the unit installed, collect its current
    // scope. (Per spec: mv is per (agent, unit) pair; we refuse if an
    // agent has the unit in multiple scopes — user must disambiguate.)
    let mut per_agent: std::collections::BTreeMap<CliAgent, Vec<CoreScope>> =
        std::collections::BTreeMap::new(); // CliAgent is Ord
    for (agent_id, sc, u) in &all {
        if u.unit_ref.unit_type != unit_type || u.unit_ref.name != name {
            continue;
        }
        let ag = match agent_id.as_str() {
            s if s == rig_adapter_claude::AGENT_ID => CliAgent::Claude,
            s if s == rig_adapter_codex::AGENT_ID => CliAgent::Codex,
            _ => continue,
        };
        if let Some(wanted) = &filter_agents {
            if !wanted.contains(&ag) {
                continue;
            }
        }
        per_agent.entry(ag).or_default().push(*sc);
    }

    if per_agent.is_empty() {
        eprintln!("{target} is not installed in any scope");
        std::process::exit(30);
    }

    // Pre-flight pass: refuse if target scope already has a
    // conflicting (type, name) for any agent we'd touch, and refuse
    // when a single agent has it in multiple scopes (ambiguous).
    for (ag, scopes) in &per_agent {
        let distinct: std::collections::HashSet<CoreScope> = scopes.iter().copied().collect();
        if distinct.contains(&to) && distinct.len() == 1 {
            // Already at target — treated as noop later, not a conflict.
            continue;
        }
        if distinct.contains(&to) {
            eprintln!(
                "{target}: {} already has the unit at target scope {to}; cannot move",
                ag.id(),
            );
            std::process::exit(31);
        }
        if distinct.len() > 1 {
            eprintln!(
                "{target}: {} has the unit installed in {:?}; --from flag (reserved) required to disambiguate",
                ag.id(),
                distinct.iter().map(|s| scope_slug(*s)).collect::<Vec<_>>(),
            );
            std::process::exit(32);
        }
    }

    let unit_ref = UnitRef::new(unit_type, name.to_owned());
    let mut any_moved = false;
    let mut any_failed = false;
    let mut any_noop = false;

    for (ag, scopes) in &per_agent {
        let from: CoreScope = scopes[0];

        // Noop: already at target.
        if from == to {
            println!("{target} [{}] already at {to}", ag.id());
            any_noop = true;
            continue;
        }

        let adapter = ag.adapter();
        if !adapter.capabilities().contains(&unit_type) {
            eprintln!("  {}  {from}→{to}  [unsupported unit type]", ag.id());
            any_failed = true;
            continue;
        }

        // Pre-fetch the source lockfile entry so we can carry
        // `source` / `source_sha` forward.
        let src_lock = match store::load_lockfile(from) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("  {}  {from}→{to}  [lockfile read failed: {e}]", ag.id());
                any_failed = true;
                continue;
            }
        };
        let src_entry = src_lock
            .entries
            .iter()
            .find(|e| {
                e.unit_type == unit_type
                    && e.agent.as_str() == ag.id()
                    && e.scope == from
                    && lock_entry_name(e) == name
            })
            .cloned();

        // Read the unit back via the adapter, then install into target.
        let unit = match adapter.read_local(&unit_ref, from) {
            Ok(u) => u,
            Err(e) => {
                eprintln!("  {}  {from}→{to}  [read_local failed: {e}]", ag.id());
                any_failed = true;
                continue;
            }
        };

        // Preserve the "disabled" state across the move. If the
        // source unit is disabled, we need to toggle after install
        // (install always writes the enabled form).
        let was_disabled = adapter
            .is_enabled(&unit_ref, from)
            .map(|e| !e)
            .unwrap_or(false);

        // Step 2: write target.
        let receipt = match adapter.install(&unit, to) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {}  {from}→{to}  [install failed: {e}]", ag.id());
                any_failed = true;
                continue;
            }
        };

        // Step 3: commit target lockfile.
        let id = src_entry
            .as_ref()
            .map(|e| e.id.clone())
            .unwrap_or_else(|| format!("{}/{}", type_slug(unit_type), name));
        let source = src_entry
            .as_ref()
            .map(|e| e.source.clone())
            .unwrap_or_else(|| Source::Local {
                path: name.to_owned(),
            });
        let source_sha = src_entry
            .as_ref()
            .map(|e| e.source_sha.clone())
            .unwrap_or_else(|| receipt.install_sha.clone());
        let native_name = if unit_type == UnitType::Mcp {
            Some(receipt.unit_ref.name.clone())
        } else {
            None
        };
        let extra = src_entry
            .as_ref()
            .map(|e| e.extra.clone())
            .unwrap_or_default();

        let mut target_lock = match store::load_lockfile(to) {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "  {}  {from}→{to}  [target lockfile read failed: {e}]",
                    ag.id()
                );
                any_failed = true;
                continue;
            }
        };
        target_lock
            .entries
            .retain(|e| !(e.id == id && e.agent == receipt.agent && e.scope == to));
        target_lock.entries.push(LockEntry {
            id: id.clone(),
            unit_type,
            source,
            source_sha,
            install_sha: receipt.install_sha.clone(),
            agent: receipt.agent.clone(),
            scope: to,
            path: receipt.paths.first().cloned().unwrap_or_default(),
            native_name,
            extra,
        });
        if let Err(e) = store::save_lockfile(to, &target_lock) {
            eprintln!(
                "  {}  {from}→{to}  [target lockfile write failed: {e}]",
                ag.id()
            );
            any_failed = true;
            continue;
        }

        // Re-apply disabled state at target, if applicable.
        if was_disabled {
            if let Err(e) = adapter.set_enabled(&unit_ref, to, false) {
                eprintln!(
                    "  {}  {from}→{to}  [warning: could not re-disable at target: {e}]",
                    ag.id()
                );
            }
        }

        // --- crash window between step 3 and 4 ---

        // Step 4: remove source bytes.
        if let Err(e) = adapter.uninstall(&unit_ref, from) {
            eprintln!(
                "  {}  {from}→{to}  [source uninstall failed: {e}; run `rig doctor` to reconcile]",
                ag.id()
            );
            any_failed = true;
            continue;
        }

        // --- crash window between step 4 and 5 ---

        // Step 5: drop source lockfile entry.
        let mut src_lock_mut = src_lock;
        src_lock_mut
            .entries
            .retain(|e| !(e.id == id && e.agent.as_str() == ag.id() && e.scope == from));
        if let Err(e) = store::save_lockfile(from, &src_lock_mut) {
            eprintln!(
                "  {}  {from}→{to}  [source lockfile write failed: {e}; run `rig doctor --fix`]",
                ag.id()
            );
            any_failed = true;
            continue;
        }

        println!(
            "moved {target} [{}] {from} -> {to}  install_sha={}",
            ag.id(),
            receipt.install_sha
        );
        any_moved = true;
    }

    // Exit-code policy (spec §8):
    //  0  all moved (or noop), no failures
    //  33 partial failure: some moved, others failed
    //  34 no moves happened and we hit an I/O failure
    if any_failed && any_moved {
        std::process::exit(33);
    }
    if any_failed && !any_moved {
        std::process::exit(34);
    }
    let _ = any_noop;
    Ok(())
}

fn type_slug(t: UnitType) -> &'static str {
    match t {
        UnitType::Skill => "skill",
        UnitType::Mcp => "mcp",
        UnitType::Rule => "rule",
        UnitType::Hook => "hook",
        UnitType::Command => "command",
        UnitType::Subagent => "subagent",
        UnitType::Plugin => "plugin",
    }
}

fn scope_slug(s: CoreScope) -> &'static str {
    match s {
        CoreScope::Global => "global",
        CoreScope::Project => "project",
        CoreScope::Local => "local",
    }
}

fn parse_type(s: &str) -> Result<UnitType> {
    Ok(match s {
        "skill" => UnitType::Skill,
        "mcp" => UnitType::Mcp,
        "rule" => UnitType::Rule,
        "hook" => UnitType::Hook,
        "command" => UnitType::Command,
        "subagent" => UnitType::Subagent,
        "plugin" => UnitType::Plugin,
        other => bail!("unknown unit type `{other}`"),
    })
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_format() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(human_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn matches_query_name_and_type() {
        assert!(matches_query("demo", UnitType::Skill, "my-demo"));
        assert!(matches_query("DEMO", UnitType::Skill, "my-demo"));
        assert!(matches_query("skill", UnitType::Skill, "anything"));
        assert!(!matches_query("zzz", UnitType::Rule, "my-demo"));
    }

    #[test]
    fn title_case_basic() {
        assert_eq!(title_case("foo-bar"), "Foo Bar");
        assert_eq!(title_case("hello_world"), "Hello World");
        assert_eq!(title_case("demo"), "Demo");
    }

    #[test]
    fn scope_slug_roundtrip() {
        assert_eq!(scope_slug(CoreScope::Global), "global");
        assert_eq!(scope_slug(CoreScope::Project), "project");
    }

    use std::path::Path;
    use std::sync::Mutex;
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    fn tempdir(tag: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let p = std::env::temp_dir().join(format!(
            "rig-cli-drift-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn with_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("HOME");
        std::env::set_var("HOME", home);
        let r = f();
        match prev {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        r
    }

    fn sample_rule() -> Unit {
        Unit::Rule(rig_core::unit::Rule {
            name: "r".into(),
            description: None,
            body: "original\n".into(),
            placement: Default::default(),
        })
    }

    fn sample_rule_v2() -> Unit {
        Unit::Rule(rig_core::unit::Rule {
            name: "r".into(),
            description: None,
            body: "upstream-v2\n".into(),
            placement: Default::default(),
        })
    }

    #[test]
    fn on_drift_keep_skips_write() {
        let tmp = tempdir("keep");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let r = adapter.install(&sample_rule(), CoreScope::Global).unwrap();
            // Tamper with local file.
            std::fs::write(&r.paths[0], b"tampered\n").unwrap();
            let unit_ref = UnitRef::new(UnitType::Rule, "r".to_owned());
            let out = apply_with_drift_resolution(
                &adapter,
                &sample_rule_v2(),
                &unit_ref,
                CoreScope::Global,
                Some(r.install_sha.clone()),
                OnDrift::Keep,
            )
            .unwrap();
            assert!(out.is_none());
            // Local tamper preserved.
            assert_eq!(std::fs::read(&r.paths[0]).unwrap(), b"tampered\n");
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn on_drift_overwrite_writes() {
        let tmp = tempdir("over");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let r = adapter.install(&sample_rule(), CoreScope::Global).unwrap();
            std::fs::write(&r.paths[0], b"tampered\n").unwrap();
            let unit_ref = UnitRef::new(UnitType::Rule, "r".to_owned());
            let out = apply_with_drift_resolution(
                &adapter,
                &sample_rule_v2(),
                &unit_ref,
                CoreScope::Global,
                Some(r.install_sha),
                OnDrift::Overwrite,
            )
            .unwrap();
            assert!(out.is_some());
            let on_disk = std::fs::read_to_string(&r.paths[0]).unwrap();
            assert!(on_disk.contains("upstream-v2"));
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn on_drift_snapshot_then_overwrite() {
        let tmp = tempdir("snap");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let r = adapter.install(&sample_rule(), CoreScope::Global).unwrap();
            std::fs::write(&r.paths[0], b"tampered\n").unwrap();
            let unit_ref = UnitRef::new(UnitType::Rule, "r".to_owned());
            let _ = apply_with_drift_resolution(
                &adapter,
                &sample_rule_v2(),
                &unit_ref,
                CoreScope::Global,
                Some(r.install_sha),
                OnDrift::SnapshotThenOverwrite,
            )
            .unwrap();
            // Incoming bytes on target path.
            assert!(std::fs::read_to_string(&r.paths[0])
                .unwrap()
                .contains("upstream-v2"));
            // Fix 2: backup lives under <scope>/.rig/snapshots/<ts>/...
            let snap_root = store::scope_dir(CoreScope::Global)
                .unwrap()
                .join("snapshots");
            assert!(
                snap_root.is_dir(),
                "no snapshots dir at {}",
                snap_root.display()
            );
            // Walk and collect all files under snap_root.
            let mut found_tampered = false;
            fn walk(dir: &Path, found: &mut bool) {
                for e in std::fs::read_dir(dir).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, found);
                    } else if let Ok(b) = std::fs::read(&p) {
                        if b == b"tampered\n" {
                            *found = true;
                        }
                    }
                }
            }
            walk(&snap_root, &mut found_tampered);
            assert!(found_tampered, "tampered bytes not found in snapshot tree");
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn links_roundtrip() {
        let tmp = tempdir("links");
        with_home(&tmp, || {
            let scope = CoreScope::Global;
            let entry = store::LinkEntry {
                agent: "claude".into(),
                name: "demo".into(),
                unit_type: UnitType::Skill,
                source: tmp.join("demo"),
            };
            store::save_links(
                scope,
                &store::Links {
                    entries: vec![entry.clone()],
                },
            )
            .unwrap();
            let loaded = store::load_links(scope).unwrap();
            assert_eq!(loaded.entries.len(), 1);
            assert_eq!(loaded.entries[0].name, "demo");
            assert_eq!(loaded.entries[0].unit_type, UnitType::Skill);
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn link_list_unlink_integration() {
        let tmp = tempdir("link-int");
        with_home(&tmp, || {
            // Create a source skill directory.
            let src = tmp.join("my-demo");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("SKILL.md"),
                "---\nname: my-demo\ndescription: d\n---\nbody\n",
            )
            .unwrap();

            link(&src, CoreScope::Global, &[CliAgent::Claude], false).unwrap();

            // links.toml should contain the entry.
            let l = store::load_links(CoreScope::Global).unwrap();
            assert_eq!(l.entries.len(), 1);
            assert_eq!(l.entries[0].name, "my-demo");

            // `collect_all` (the function that powers `list`) should
            // surface the linked skill — either via the adapter's
            // native list or via the links.toml merge path.
            let all = collect_all(&[CoreScope::Global]).unwrap();
            assert!(all.iter().any(|(_, _, u)| u.unit_ref.name == "my-demo"));

            // Unlink removes the symlink and the entry.
            unlink(
                "skill/my-demo",
                Some(&[CliAgent::Claude]),
                CoreScope::Global,
            )
            .unwrap();
            let l2 = store::load_links(CoreScope::Global).unwrap();
            assert!(l2.entries.is_empty());
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn on_drift_cancel_aborts() {
        let tmp = tempdir("cancel");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let r = adapter.install(&sample_rule(), CoreScope::Global).unwrap();
            std::fs::write(&r.paths[0], b"tampered\n").unwrap();
            let unit_ref = UnitRef::new(UnitType::Rule, "r".to_owned());
            let res = apply_with_drift_resolution(
                &adapter,
                &sample_rule_v2(),
                &unit_ref,
                CoreScope::Global,
                Some(r.install_sha),
                OnDrift::Cancel,
            );
            assert!(res.is_err());
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    // ---------- mv wedge tests (ENABLE-DISABLE-MV.md §§8, 10-12) ----------

    fn sample_skill(name: &str) -> Unit {
        Unit::Skill(rig_core::unit::Skill {
            name: name.into(),
            description: "sample".into(),
            extra_frontmatter: Default::default(),
            body: format!("body for {name}\n"),
            resources: Vec::new(),
        })
    }

    /// Seed a lockfile entry for `(unit_type, name, agent, scope)`
    /// using the receipt that `adapter.install` returned. Mirrors
    /// `upsert_lock` but without pulling in a full Source; we use a
    /// `Source::Local` stand-in so the test is hermetic.
    fn seed_lock(scope: CoreScope, receipt: &Receipt, unit_type: UnitType) {
        let name = receipt.unit_ref.name.clone();
        let source = Source::Local { path: name.clone() };
        let id = format!("{}/{}", type_slug(unit_type), source);
        let mut lock = store::load_lockfile(scope).unwrap();
        lock.entries.push(LockEntry {
            id,
            unit_type,
            source,
            source_sha: receipt.install_sha.clone(),
            install_sha: receipt.install_sha.clone(),
            agent: receipt.agent.clone(),
            scope,
            path: receipt.paths.first().cloned().unwrap_or_default(),
            native_name: if unit_type == UnitType::Mcp {
                Some(name)
            } else {
                None
            },
            extra: Default::default(),
        });
        store::save_lockfile(scope, &lock).unwrap();
    }

    /// Skill mv roundtrip: install global, move to project, assert
    /// install_sha preserved, lockfile flipped, only one scope has
    /// the unit on disk.
    #[test]
    fn mv_skill_global_to_project() {
        let tmp = tempdir("mv-skill");
        let home = tmp.join("home");
        let proj = tmp.join("proj");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        with_home(&home, || {
            let prev_cwd = std::env::current_dir().unwrap();
            std::env::set_current_dir(&proj).unwrap();

            let adapter = ClaudeAdapter::new();
            let r = adapter
                .install(&sample_skill("mv-me"), CoreScope::Global)
                .unwrap();
            seed_lock(CoreScope::Global, &r, UnitType::Skill);
            let original_sha = r.install_sha.clone();

            mv("skill/mv-me", CoreScope::Project, Some(&[CliAgent::Claude])).unwrap();

            // Disk: source gone, target populated.
            let global_list = adapter.list(CoreScope::Global).unwrap();
            let project_list = adapter.list(CoreScope::Project).unwrap();
            assert!(!global_list.iter().any(|u| u.unit_ref.name == "mv-me"));
            assert!(project_list.iter().any(|u| u.unit_ref.name == "mv-me"));

            // Lockfile: source entry dropped, target entry has same install_sha.
            let glock = store::load_lockfile(CoreScope::Global).unwrap();
            let plock = store::load_lockfile(CoreScope::Project).unwrap();
            assert!(
                !glock
                    .entries
                    .iter()
                    .any(|e| e.unit_type == UnitType::Skill && lock_entry_name(e) == "mv-me"),
                "source lock entry not dropped"
            );
            let target = plock
                .entries
                .iter()
                .find(|e| e.unit_type == UnitType::Skill && lock_entry_name(e) == "mv-me")
                .expect("target lock entry missing");
            assert_eq!(
                target.install_sha, original_sha,
                "install_sha not preserved across mv"
            );

            std::env::set_current_dir(prev_cwd).ok();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Already-at-target mv is a noop (prints and exits Ok).
    #[test]
    fn mv_already_at_target_noop() {
        let tmp = tempdir("mv-noop");
        let home = tmp.join("home");
        let proj = tmp.join("proj");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        with_home(&home, || {
            let prev_cwd = std::env::current_dir().unwrap();
            std::env::set_current_dir(&proj).unwrap();

            let adapter = ClaudeAdapter::new();
            let r = adapter
                .install(&sample_skill("stayput"), CoreScope::Global)
                .unwrap();
            seed_lock(CoreScope::Global, &r, UnitType::Skill);

            // Target = Global (already there). Must not fail.
            mv(
                "skill/stayput",
                CoreScope::Global,
                Some(&[CliAgent::Claude]),
            )
            .unwrap();

            // Disk unchanged.
            let global_list = adapter.list(CoreScope::Global).unwrap();
            assert!(global_list.iter().any(|u| u.unit_ref.name == "stayput"));

            std::env::set_current_dir(prev_cwd).ok();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Disabled unit: after mv, the unit is still disabled at target.
    #[test]
    fn mv_preserves_disabled_state() {
        let tmp = tempdir("mv-disabled");
        let home = tmp.join("home");
        let proj = tmp.join("proj");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        with_home(&home, || {
            let prev_cwd = std::env::current_dir().unwrap();
            std::env::set_current_dir(&proj).unwrap();

            let adapter = ClaudeAdapter::new();
            let unit_ref = UnitRef::new(UnitType::Rule, "r".to_owned());
            let r = adapter.install(&sample_rule(), CoreScope::Global).unwrap();
            seed_lock(CoreScope::Global, &r, UnitType::Rule);
            adapter
                .set_enabled(&unit_ref, CoreScope::Global, false)
                .unwrap();
            assert!(!adapter.is_enabled(&unit_ref, CoreScope::Global).unwrap());

            mv("rule/r", CoreScope::Project, Some(&[CliAgent::Claude])).unwrap();

            // Target must report disabled.
            let is_enabled_target = adapter
                .is_enabled(&unit_ref, CoreScope::Project)
                .unwrap_or(true);
            assert!(!is_enabled_target, "disabled state lost across mv");

            std::env::set_current_dir(prev_cwd).ok();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Doctor --fix drops a stale lockfile entry (simulated crash
    /// between steps 4 and 5 of mv: unit bytes moved, but source
    /// lockfile still has the entry).
    #[test]
    fn doctor_fix_drops_stale_lock_entry() {
        let tmp = tempdir("doc-stale");
        let home = tmp.join("home");
        let proj = tmp.join("proj");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        with_home(&home, || {
            let prev_cwd = std::env::current_dir().unwrap();
            std::env::set_current_dir(&proj).unwrap();

            let adapter = ClaudeAdapter::new();
            let r = adapter
                .install(&sample_skill("orphan"), CoreScope::Project)
                .unwrap();
            // Seed lockfile entry claiming the unit lives in GLOBAL,
            // even though it's only on disk in PROJECT. This is
            // precisely the "stale lock entry" crash state.
            let mut r_for_lock = r.clone();
            r_for_lock.scope = CoreScope::Global;
            seed_lock(CoreScope::Global, &r_for_lock, UnitType::Skill);
            // Also seed the real project entry so doctor doesn't
            // complain about the *project* side being split.
            seed_lock(CoreScope::Project, &r, UnitType::Skill);

            // Pre-fix: global lockfile has an entry.
            let g_before = store::load_lockfile(CoreScope::Global).unwrap();
            assert_eq!(g_before.entries.len(), 1);

            doctor(true).unwrap();

            // Post-fix: stale entry dropped.
            let g_after = store::load_lockfile(CoreScope::Global).unwrap();
            assert!(
                g_after.entries.is_empty(),
                "doctor --fix should have dropped the stale entry"
            );

            std::env::set_current_dir(prev_cwd).ok();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Doctor without --fix does NOT auto-resolve split state (unit
    /// present on disk in two scopes while lockfile only claims one).
    #[test]
    fn doctor_reports_but_does_not_fix_split_state() {
        let tmp = tempdir("doc-split");
        let home = tmp.join("home");
        let proj = tmp.join("proj");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        with_home(&home, || {
            let prev_cwd = std::env::current_dir().unwrap();
            std::env::set_current_dir(&proj).unwrap();

            let adapter = ClaudeAdapter::new();
            // Install into both global and project.
            let r_g = adapter
                .install(&sample_skill("twin"), CoreScope::Global)
                .unwrap();
            let _r_p = adapter
                .install(&sample_skill("twin"), CoreScope::Project)
                .unwrap();
            // Lockfile only covers GLOBAL → the PROJECT copy is the
            // "split-state" tail of a crashed mv (target written,
            // source not yet uninstalled — but only mirrored in lock).
            seed_lock(CoreScope::Global, &r_g, UnitType::Skill);

            doctor(true).unwrap();

            // Even with --fix, both on-disk copies should survive
            // (split state is report-only).
            let global_list = adapter.list(CoreScope::Global).unwrap();
            let project_list = adapter.list(CoreScope::Project).unwrap();
            assert!(global_list.iter().any(|u| u.unit_ref.name == "twin"));
            assert!(project_list.iter().any(|u| u.unit_ref.name == "twin"));

            std::env::set_current_dir(prev_cwd).ok();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// `collect_all` must hide MCP entries the adapter reports if
    /// Rig's lockfile doesn't own them — spec MCP-SUPPORT.md §6 /
    /// ENABLE-DISABLE-MV.md wedge A deferred.
    #[test]
    fn foreign_mcp_filtered_from_list() {
        let tmp = tempdir("foreign-mcp");
        with_home(&tmp, || {
            // Seed `~/.claude.json` with a foreign MCP entry.
            let claude_json = tmp.join(".claude.json");
            std::fs::write(
                &claude_json,
                r#"{"mcpServers":{"foreign":{"type":"stdio","command":"echo","args":[]}}}"#,
            )
            .unwrap();
            // No rig.lock → foreign MCP is NOT in Rig's lockfile.
            let collected = collect_all(&[CoreScope::Global]).unwrap();
            let leaked = collected
                .iter()
                .any(|(_, _, u)| u.unit_ref.unit_type == UnitType::Mcp);
            assert!(!leaked, "foreign MCP leaked into collect_all");
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    // ---------- Fix 1: MCP goes through native_for for both adapters ----

    #[test]
    fn native_for_handles_mcp_both_agents() {
        let m = rig_core::unit::Mcp {
            name: "demo".into(),
            description: Some("demo".into()),
            transport: rig_core::unit::mcp::Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "@demo/mcp".into()],
            },
            env: Vec::new(),
            metadata: Default::default(),
        };
        let unit = Unit::Mcp(m);

        let claude = ClaudeAdapter::new();
        let lay_c = native_for(&claude, &unit).expect("claude native_for Unit::Mcp");
        assert!(!lay_c.files.is_empty(), "claude MCP layout empty");

        let codex = CodexAdapter::new();
        let lay_x = native_for(&codex, &unit).expect("codex native_for Unit::Mcp");
        assert!(!lay_x.files.is_empty(), "codex MCP layout empty");
    }

    // ---------- Fix 4: init-mcp writes a parseable mcp.toml ----

    #[test]
    fn init_mcp_stdio_writes_parseable() {
        let tmp = tempdir("init-mcp");
        init_mcp("demo", Some(&tmp), CliMcpTransport::Stdio).unwrap();
        let body = std::fs::read_to_string(tmp.join("demo").join("mcp.toml")).unwrap();
        assert!(body.contains("name = \"demo\""));
        assert!(body.contains("kind = \"stdio\""));
        // Full parse via the canonical source parser.
        let mcp = rig_core::unit::mcp::parse_source(&body).expect("mcp.toml parses");
        assert_eq!(mcp.name, "demo");
        assert!(matches!(
            mcp.transport,
            rig_core::unit::mcp::Transport::Stdio { .. }
        ));
        std::fs::remove_dir_all(&tmp).ok();
    }

    // ---------- Fix 3: inspect prints expected frontmatter + files ----

    #[test]
    fn inspect_local_skill_smoke() {
        let tmp = tempdir("inspect");
        let skill_dir = tmp.join("xyz-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: xyz-skill\ndescription: inspect-me\n---\n\nbody here\n",
        )
        .unwrap();
        // Just make sure inspect() runs without error on a local path.
        // (Output is captured by println; we don't assert on stdout.)
        let src = format!("local:{}", skill_dir.display());
        inspect(&src, false, None, None).expect("inspect runs");
        // JSON mode should also succeed.
        inspect(&src, true, None, None).expect("inspect --json runs");
        std::fs::remove_dir_all(&tmp).ok();
    }

    // ---------- Fix 5: search matches description ----

    #[test]
    fn search_matches_description_substring() {
        let tmp = tempdir("search-desc");
        with_home(&tmp, || {
            // Install a skill whose description contains "xyzzy".
            let adapter = ClaudeAdapter::new();
            let u = Unit::Skill(rig_core::unit::Skill {
                name: "needle".into(),
                description: "contains xyzzy marker for search test".into(),
                extra_frontmatter: Default::default(),
                body: "body".into(),
                resources: Vec::new(),
            });
            adapter.install(&u, CoreScope::Global).unwrap();

            // Verify the description-match path via the same helpers.
            let all = collect_all(&[CoreScope::Global]).unwrap();
            let hit = all.iter().find(|(_, _, u)| u.unit_ref.name == "needle");
            assert!(hit.is_some(), "skill not listed: {all:?}");
            let (_agent, sc, unit) = hit.unwrap();
            let local = adapter.read_local(&unit.unit_ref, *sc).unwrap();
            let (desc, _body) = unit_description_body(&local);
            assert!(
                desc.to_lowercase().contains("xyzzy"),
                "description missing marker: {desc:?}"
            );
            // Name-based query should NOT match (no xyzzy in name);
            // description-based query MUST match via the new logic.
            assert!(!matches_query("xyzzy", UnitType::Skill, "needle"));
        });
        std::fs::remove_dir_all(&tmp).ok();
    }
}
