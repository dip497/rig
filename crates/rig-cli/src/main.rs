//! `rig` CLI — install / sync / status / list / uninstall for Claude
//! Code. Manifest + lockfile live at `<scope>/.rig/`.

mod store;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use rig_adapter_claude::{
    ClaudeAdapter, CommandConverter, RuleConverter, SkillConverter, SubagentConverter,
};
use rig_adapter_codex::CodexAdapter;
use rig_core::adapter::{Adapter, Receipt, UnitRef};
use rig_core::converter::Converter;
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
    },
    /// Install everything declared in `rig.toml`, writing the lockfile.
    Sync {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
    },
    /// Report drift between `rig.lock` and disk.
    Status {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
    },
    /// List Rig-installed units for Claude Code.
    List {
        #[arg(long, value_enum, default_value_t = CliScope::Project)]
        scope: CliScope,
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
}

#[derive(Copy, Clone, ValueEnum)]
enum CliScope {
    Global,
    Project,
}

impl From<CliScope> for CoreScope {
    fn from(s: CliScope) -> Self {
        match s {
            CliScope::Global => Self::Global,
            CliScope::Project => Self::Project,
        }
    }
}

#[derive(Copy, Clone, ValueEnum, PartialEq, Eq)]
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
}

#[derive(Copy, Clone, ValueEnum)]
enum CliUnitType {
    Skill,
    Rule,
    Command,
    Subagent,
}

impl From<CliUnitType> for UnitType {
    fn from(t: CliUnitType) -> Self {
        match t {
            CliUnitType::Skill => Self::Skill,
            CliUnitType::Rule => Self::Rule,
            CliUnitType::Command => Self::Command,
            CliUnitType::Subagent => Self::Subagent,
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
        } => install(
            &source,
            scope.into(),
            as_type.map(Into::into),
            &agent,
            !no_manifest,
        ),
        Command::Sync { scope } => sync(scope.into()),
        Command::Status { scope } => status(scope.into()),
        Command::List { scope } => list(scope.into()),
        Command::Uninstall { target, scope } => uninstall(&target, scope.into()),
        Command::Pack { path, out } => pack(&path, out.as_deref()),
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
        let receipt = adapter
            .install(&unit, scope)
            .with_context(|| format!("installing into {} ({scope})", adapter.agent()))?;
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
        let id = rig_core::agent::AgentId::new(match ag {
            CliAgent::Claude => rig_adapter_claude::AGENT_ID,
            CliAgent::Codex => rig_adapter_codex::AGENT_ID,
        });
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
    });
    store::save_lockfile(scope, &lock)
}

fn lock_id(t: UnitType, source: &Source) -> String {
    format!("{}/{}", type_slug(t), source)
}

fn sync(scope: CoreScope) -> Result<()> {
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
                let receipt = adapter.install(&unit, scope).with_context(|| {
                    format!(
                        "bundle `{name}`: installing `{src}` into {}",
                        adapter.agent()
                    )
                })?;
                println!(
                    "  ✓ {}/{}  ({src}) → {}",
                    type_slug(ty),
                    receipt.unit_ref.name,
                    adapter.agent(),
                );
                new_lock.entries.push(LockEntry {
                    id: lock_id(ty, &source),
                    unit_type: ty,
                    source: source.clone(),
                    source_sha: source_sha.clone(),
                    install_sha: receipt.install_sha,
                    agent: receipt.agent,
                    scope,
                    path: receipt.paths.into_iter().next().unwrap_or_default(),
                });
                installed += 1;
            }
        }
    }

    store::save_lockfile(scope, &new_lock)?;
    println!(
        "synced {installed} unit(s); lockfile at {}",
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

fn status(scope: CoreScope) -> Result<()> {
    let lock = store::load_lockfile(scope)?;
    if lock.entries.is_empty() {
        println!(
            "no lockfile entries at {}",
            store::lockfile_path(scope)?.display()
        );
        return Ok(());
    }
    let mut clean = 0;
    let mut checked = 0;
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
        // The installed path's stem is the canonical unit name for
        // every type we support today (skill dir is `<name>/SKILL.md`
        // → parent stem; single-file types use `<name>.md`).
        let name = if e.unit_type == UnitType::Skill {
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
        let (state, _) = adapter
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
        let marker = match state {
            DriftState::Clean => {
                clean += 1;
                "clean"
            }
            DriftState::LocalDrift => "local-drift",
            DriftState::UpstreamDrift => "upstream-drift",
            DriftState::BothDrift => "both-drift",
            DriftState::Orphan => "orphan",
            DriftState::Missing => "missing",
        };
        println!(
            "  {:<14} {}/{}  [{}]",
            marker,
            type_slug(e.unit_type),
            name,
            e.agent
        );
    }
    println!("{clean}/{checked} clean");
    Ok(())
}

fn list(scope: CoreScope) -> Result<()> {
    let mut any = false;
    for adapter in [
        Box::new(ClaudeAdapter::new()) as Box<dyn Adapter>,
        Box::new(CodexAdapter::new()),
    ] {
        let units = adapter
            .list(scope)
            .with_context(|| format!("listing {} ({scope})", adapter.agent()))?;
        if units.is_empty() {
            continue;
        }
        any = true;
        for u in units {
            println!(
                "{}/{} ({} file{}) [{}]",
                type_slug(u.unit_ref.unit_type),
                u.unit_ref.name,
                u.paths.len(),
                if u.paths.len() == 1 { "" } else { "s" },
                adapter.agent(),
            );
        }
    }
    if !any {
        println!("no units installed in any agent ({scope})");
    }
    Ok(())
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
