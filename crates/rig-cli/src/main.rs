//! `rig` CLI — install / sync / status / list / uninstall for Claude
//! Code. Manifest + lockfile live at `<scope>/.rig/`.

mod store;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

use rig_adapter_claude::{
    ClaudeAdapter, CommandConverter, RuleConverter, SkillConverter, SubagentConverter,
};
use rig_adapter_codex::CodexAdapter;
use rig_core::adapter::{Adapter, InstalledUnit, Receipt, UnitRef};
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
    /// Substring search across installed units (name + type).
    Search {
        query: String,
        #[arg(long, value_enum, default_value_t = CliScopeAll::All)]
        scope: CliScopeAll,
        #[arg(long)]
        json: bool,
    },
    /// Per-agent × scope breakdown: counts + disk usage.
    Stats {
        #[arg(long, value_enum, default_value_t = CliScopeAll::All)]
        scope: CliScopeAll,
        #[arg(long)]
        json: bool,
    },
    /// Audit: duplicates across agents, broken symlinks.
    Doctor,
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

/// Scope selector that admits "all".
#[derive(Copy, Clone, ValueEnum)]
enum CliScopeAll {
    Global,
    Project,
    All,
}

impl CliScopeAll {
    fn scopes(self) -> Vec<CoreScope> {
        match self {
            Self::Global => vec![CoreScope::Global],
            Self::Project => vec![CoreScope::Project],
            Self::All => vec![CoreScope::Global, CoreScope::Project],
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

    fn id(self) -> &'static str {
        match self {
            Self::Claude => rig_adapter_claude::AGENT_ID,
            Self::Codex => rig_adapter_codex::AGENT_ID,
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
        Command::Search { query, scope, json } => search(&query, scope, json),
        Command::Stats { scope, json } => stats(scope, json),
        Command::Doctor => doctor(),
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
}

fn list(scope: CoreScope, json: bool) -> Result<()> {
    let all = collect_all(&[scope])?;
    if json {
        let rows: Vec<ListEntry<'_>> = all
            .iter()
            .map(|(agent, sc, u)| ListEntry {
                agent: agent.as_str(),
                unit_type: type_slug(u.unit_ref.unit_type),
                name: u.unit_ref.name.clone(),
                scope: scope_slug(*sc),
                paths: u.paths.iter().map(|p| p.display().to_string()).collect(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if all.is_empty() {
        println!("no units installed in any agent ({scope})");
        return Ok(());
    }
    for (agent, _sc, u) in &all {
        println!(
            "{}/{} ({} file{}) [{}]",
            type_slug(u.unit_ref.unit_type),
            u.unit_ref.name,
            u.paths.len(),
            if u.paths.len() == 1 { "" } else { "s" },
            agent,
        );
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
        }
        Ok(())
    }
}

fn link_target(ag: CliAgent, scope: CoreScope, name: &str) -> Result<PathBuf> {
    let root: PathBuf = match scope {
        CoreScope::Global => rig_fs::home_dir().context("discovering home dir")?,
        CoreScope::Project => PathBuf::from("."),
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
}

fn search(query: &str, scope: CliScopeAll, json: bool) -> Result<()> {
    let all = collect_all(&scope.scopes())?;
    let matches: Vec<_> = all
        .into_iter()
        .filter(|(_, _, u)| matches_query(query, u.unit_ref.unit_type, &u.unit_ref.name))
        .collect();

    if json {
        let rows: Vec<SearchRow<'_>> = matches
            .iter()
            .map(|(agent, sc, u)| SearchRow {
                agent: agent.as_str(),
                unit_type: type_slug(u.unit_ref.unit_type),
                name: u.unit_ref.name.clone(),
                scope: scope_slug(*sc),
                paths: u.paths.iter().map(|p| p.display().to_string()).collect(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if matches.is_empty() {
        println!("no matches");
        return Ok(());
    }
    for (agent, sc, u) in &matches {
        let path = u
            .paths
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        println!(
            "{}/{}  [{}]  ({})  {}",
            type_slug(u.unit_ref.unit_type),
            u.unit_ref.name,
            agent,
            scope_slug(*sc),
            path
        );
    }
    Ok(())
}

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

fn doctor() -> Result<()> {
    use std::collections::HashMap;

    let all = collect_all(&[CoreScope::Global, CoreScope::Project])?;

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

    if dup_count == 0 && broken_count == 0 {
        println!("all clean");
    } else {
        println!("{dup_count} duplicates, {broken_count} broken symlinks");
    }
    Ok(())
}

// ---------- Shared helpers ----------

/// List every installed unit across both adapters and the given scopes.
fn collect_all(
    scopes: &[CoreScope],
) -> Result<Vec<(rig_core::agent::AgentId, CoreScope, InstalledUnit)>> {
    let mut out = Vec::new();
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
                out.push((adapter.agent(), sc, u));
            }
        }
    }
    Ok(out)
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
}
