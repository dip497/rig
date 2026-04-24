//! Sync, search, stats, and doctor helpers — GUI flavour.
//!
//! These mirror CLI `sync` / `search` / `stats` / `doctor` in
//! `crates/rig-cli/src/main.rs`, adapted to (a) take an explicit project
//! root rather than using cwd, (b) operate through the GUI's
//! `AppState` adapter handles, and (c) return structured DTOs instead
//! of printing. `diff-per-file` is explicitly rejected — it's an
//! interactive-TTY mode and has no GUI analogue in M1.
//!
//! Keep logic here in lockstep with the CLI; if a bug is found in one,
//! fix both.
//!
//! NOTE: all writes stay thin — we reuse adapter.install / uninstall
//! and the per-scope store helpers directly. No cross-adapter imports
//! and no new I/O primitives.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rig_adapter_claude::{
    CommandConverter as CCommandConverter, RuleConverter as CRuleConverter,
    SkillConverter as CSkillConverter, SubagentConverter as CSubagentConverter,
};
use rig_core::adapter::{Adapter, InstalledUnit, Receipt, UnitRef};
use rig_core::agent::AgentId;
use rig_core::converter::{Converter, NativeLayout};
use rig_core::drift::DriftState;
use rig_core::lockfile::LockEntry;
use rig_core::manifest::Bundle;
use rig_core::scope::Scope;
use rig_core::source::Source;
use rig_core::unit::{Unit, UnitType};

use crate::dto::{
    unit_type_slug, AgentStatsDto, DoctorResultDto, DuplicateDto, DuplicateLocationDto,
    InstalledUnitDto, StatsDto, SyncResultDto, TypeStatsDto,
};
use crate::state::AppState;
use crate::store;

/// Parsed drift-mode selector accepted by the GUI.
#[derive(Copy, Clone, Debug)]
pub enum OnDriftMode {
    Keep,
    Overwrite,
    SnapshotThenOverwrite,
    Cancel,
}

pub fn parse_on_drift(s: &str) -> Result<OnDriftMode, String> {
    match s {
        "keep" => Ok(OnDriftMode::Keep),
        "overwrite" => Ok(OnDriftMode::Overwrite),
        "snapshot-then-overwrite" => Ok(OnDriftMode::SnapshotThenOverwrite),
        "cancel" => Ok(OnDriftMode::Cancel),
        "diff-per-file" => Err(
            "`diff-per-file` is a TTY-only mode; use CLI `rig sync --on-drift diff-per-file`"
                .to_owned(),
        ),
        other => Err(format!("unknown on-drift mode `{other}`")),
    }
}

fn lock_id(t: UnitType, source: &Source) -> String {
    format!("{}/{}", unit_type_slug(t), source)
}

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
    out
}

fn fetch_unit(
    source: &Source,
    as_type: Option<UnitType>,
) -> Result<(Unit, rig_core::source::Sha256), String> {
    let fetched = rig_source::fetch(source).map_err(|e| e.to_string())?;
    let unit_type = match (fetched.detected, as_type) {
        (_, Some(t)) => t,
        (Some(t), None) => t,
        (None, None) => {
            return Err("could not auto-detect unit type; specify via bundle type".to_owned());
        }
    };
    let unit = match unit_type {
        UnitType::Skill => Unit::Skill(
            CSkillConverter
                .parse_native(&fetched.native)
                .map_err(|e| e.to_string())?,
        ),
        UnitType::Rule => Unit::Rule(
            CRuleConverter
                .parse_native(&fetched.native)
                .map_err(|e| e.to_string())?,
        ),
        UnitType::Command => Unit::Command(
            CCommandConverter
                .parse_native(&fetched.native)
                .map_err(|e| e.to_string())?,
        ),
        UnitType::Subagent => Unit::Subagent(
            CSubagentConverter
                .parse_native(&fetched.native)
                .map_err(|e| e.to_string())?,
        ),
        other => return Err(format!("unit type `{other:?}` not supported in GUI sync")),
    };
    Ok((unit, fetched.source_sha))
}

fn native_for(agent: &str, unit: &Unit) -> Result<NativeLayout, String> {
    let native = match (agent, unit) {
        ("claude", Unit::Skill(u)) => CSkillConverter.to_native(u).map_err(|e| e.to_string())?,
        ("claude", Unit::Rule(u)) => CRuleConverter.to_native(u).map_err(|e| e.to_string())?,
        ("claude", Unit::Command(u)) => {
            CCommandConverter.to_native(u).map_err(|e| e.to_string())?
        }
        ("claude", Unit::Subagent(u)) => {
            CSubagentConverter.to_native(u).map_err(|e| e.to_string())?
        }
        ("codex", Unit::Skill(u)) => rig_adapter_codex::SkillConverter
            .to_native(u)
            .map_err(|e| e.to_string())?,
        ("codex", Unit::Rule(u)) => rig_adapter_codex::RuleConverter
            .to_native(u)
            .map_err(|e| e.to_string())?,
        ("codex", Unit::Command(u)) => rig_adapter_codex::CommandConverter
            .to_native(u)
            .map_err(|e| e.to_string())?,
        ("codex", Unit::Subagent(u)) => rig_adapter_codex::SubagentConverter
            .to_native(u)
            .map_err(|e| e.to_string())?,
        _ => return Err("unsupported (agent, unit) combination".to_owned()),
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

fn snapshot_current(adapter: &dyn Adapter, unit_ref: &UnitRef, scope: Scope) -> Result<(), String> {
    let listed = adapter.list(scope).unwrap_or_default();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(iu) = listed
        .iter()
        .find(|u| u.unit_ref.unit_type == unit_ref.unit_type && u.unit_ref.name == unit_ref.name)
    {
        for p in &iu.paths {
            if !p.exists() {
                continue;
            }
            let mut backup = p.clone().into_os_string();
            backup.push(format!(".rig-backup-{ts}"));
            std::fs::rename(p, PathBuf::from(&backup)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Outcome of a single (bundle entry × agent) attempt.
enum ApplyOutcome {
    Installed(Receipt),
    Skipped,
    Cancelled,
}

fn apply_with_drift_resolution(
    adapter: &dyn Adapter,
    unit: &Unit,
    unit_ref: &UnitRef,
    scope: Scope,
    prior: Option<rig_core::source::Sha256>,
    on_drift: OnDriftMode,
) -> Result<ApplyOutcome, String> {
    let incoming_native = native_for(adapter.agent().as_str(), unit)?;

    let current_native = match adapter.read_local(unit_ref, scope) {
        Ok(local) => Some(native_for(adapter.agent().as_str(), &local)?),
        Err(_) => None,
    };

    if let (Some(cur), Some(prior)) = (&current_native, &prior) {
        let cur_sha = hash_layout(cur);
        if cur_sha == *prior && hash_layout(&incoming_native) == cur_sha {
            return Ok(ApplyOutcome::Installed(Receipt {
                unit_ref: unit_ref.clone(),
                agent: adapter.agent(),
                scope,
                paths: Vec::new(),
                install_sha: cur_sha,
            }));
        }
    }

    let drift_state = match &prior {
        Some(sha) => adapter
            .detect_drift(unit_ref, scope, sha.clone(), None)
            .map(|(s, _)| s)
            .unwrap_or(DriftState::Missing),
        None => {
            if current_native.is_some() {
                DriftState::LocalDrift
            } else {
                DriftState::Missing
            }
        }
    };

    if matches!(drift_state, DriftState::Clean | DriftState::Missing) {
        let r = adapter.install(unit, scope).map_err(|e| e.to_string())?;
        return Ok(ApplyOutcome::Installed(r));
    }

    match on_drift {
        OnDriftMode::Keep => Ok(ApplyOutcome::Skipped),
        OnDriftMode::Overwrite => {
            let r = adapter.install(unit, scope).map_err(|e| e.to_string())?;
            Ok(ApplyOutcome::Installed(r))
        }
        OnDriftMode::SnapshotThenOverwrite => {
            snapshot_current(adapter, unit_ref, scope)?;
            let r = adapter.install(unit, scope).map_err(|e| e.to_string())?;
            Ok(ApplyOutcome::Installed(r))
        }
        OnDriftMode::Cancel => Ok(ApplyOutcome::Cancelled),
    }
}

fn prior_install_sha(
    lock: &rig_core::lockfile::Lockfile,
    unit_type: UnitType,
    source: &Source,
    agent_id: &str,
    scope: Scope,
) -> Option<rig_core::source::Sha256> {
    let id = lock_id(unit_type, source);
    lock.entries
        .iter()
        .find(|e| e.id == id && e.agent.as_str() == agent_id && e.scope == scope)
        .map(|e| e.install_sha.clone())
}

fn installed_to_dto(iu: InstalledUnit, agent: &str) -> InstalledUnitDto {
    InstalledUnitDto {
        agent: agent.to_owned(),
        unit_type: unit_type_slug(iu.unit_ref.unit_type).to_owned(),
        name: iu.unit_ref.name,
        paths: iu.paths,
        disabled: iu.disabled,
    }
}

/// Replicates CLI `sync()` — reads the scope manifest, walks bundles,
/// fetches each entry, applies per-agent with drift resolution, then
/// rewrites the lockfile.
pub fn sync_scope(
    state: &AppState,
    scope: Scope,
    project_root: Option<&Path>,
    on_drift: OnDriftMode,
) -> Result<SyncResultDto, String> {
    let manifest = store::load_manifest(scope, project_root).map_err(|e| e.to_string())?;
    let mut installed: Vec<InstalledUnitDto> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    if manifest.bundles.is_empty() {
        return Ok(SyncResultDto {
            installed,
            skipped,
            conflicts,
            cancelled: false,
        });
    }

    let targets: Vec<&str> = if manifest.agents.targets.is_empty() {
        vec!["claude"]
    } else {
        manifest
            .agents
            .targets
            .iter()
            .filter_map(|id| match id.as_str() {
                "claude" => Some("claude"),
                "codex" => Some("codex"),
                other => {
                    skipped.push(format!("unknown agent `{other}` in manifest"));
                    None
                }
            })
            .collect()
    };

    let prev_lock = store::load_lockfile(scope, project_root).map_err(|e| e.to_string())?;
    let mut new_lock = rig_core::lockfile::Lockfile::new();

    for (bname, bundle) in &manifest.bundles {
        for (src, ty) in iter_bundle_entries(bundle) {
            let source = match Source::parse(&src) {
                Ok(s) => s,
                Err(e) => {
                    conflicts.push(format!("bundle `{bname}`: parse `{src}`: {e}"));
                    continue;
                }
            };
            let (unit, source_sha) = match fetch_unit(&source, Some(ty)) {
                Ok(x) => x,
                Err(e) => {
                    conflicts.push(format!("bundle `{bname}`: fetch `{src}`: {e}"));
                    continue;
                }
            };

            for agent_id in &targets {
                let Some(adapter) = state.adapter_by_id(agent_id) else {
                    continue;
                };
                if !adapter.capabilities().contains(&ty) {
                    continue;
                }
                let uname = canonical_name(&unit);
                let unit_ref = UnitRef::new(ty, uname);
                let prior = prior_install_sha(&prev_lock, ty, &source, agent_id, scope);

                match apply_with_drift_resolution(adapter, &unit, &unit_ref, scope, prior, on_drift)
                {
                    Ok(ApplyOutcome::Installed(receipt)) => {
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
                            install_sha: receipt.install_sha.clone(),
                            agent: receipt.agent.clone(),
                            scope,
                            path: receipt.paths.first().cloned().unwrap_or_default(),
                            native_name,
                            extra: Default::default(),
                        });
                        installed.push(InstalledUnitDto {
                            agent: (*agent_id).to_owned(),
                            unit_type: unit_type_slug(ty).to_owned(),
                            name: receipt.unit_ref.name.clone(),
                            paths: receipt.paths,
                            disabled: false,
                        });
                    }
                    Ok(ApplyOutcome::Skipped) => {
                        skipped.push(format!(
                            "{}/{} [{}] (local-drift)",
                            unit_type_slug(ty),
                            unit_ref.name,
                            agent_id,
                        ));
                    }
                    Ok(ApplyOutcome::Cancelled) => {
                        conflicts.push(format!(
                            "cancel on drift: {}/{} [{}]",
                            unit_type_slug(ty),
                            unit_ref.name,
                            agent_id,
                        ));
                        return Ok(SyncResultDto {
                            installed,
                            skipped,
                            conflicts,
                            cancelled: true,
                        });
                    }
                    Err(e) => {
                        conflicts.push(format!(
                            "bundle `{bname}`: install `{src}` into {agent_id}: {e}"
                        ));
                    }
                }
            }
        }
    }

    // Preserve prior entries for things we didn't touch this run.
    for e in prev_lock.entries {
        let already = new_lock
            .entries
            .iter()
            .any(|n| n.id == e.id && n.agent == e.agent && n.scope == e.scope);
        if !already {
            new_lock.entries.push(e);
        }
    }
    store::save_lockfile(scope, project_root, &new_lock).map_err(|e| e.to_string())?;

    Ok(SyncResultDto {
        installed,
        skipped,
        conflicts,
        cancelled: false,
    })
}

pub fn search_units(
    state: &AppState,
    scope: Scope,
    _project_root: Option<&Path>,
    query: &str,
) -> Result<Vec<InstalledUnitDto>, String> {
    let q = query.to_lowercase();
    let mut out = Vec::new();
    for a in state.agents() {
        let agent_id = a.agent().as_str().to_owned();
        let units = a.list(scope).map_err(|e| e.to_string())?;
        for u in units {
            let slug = unit_type_slug(u.unit_ref.unit_type);
            if q.is_empty()
                || u.unit_ref.name.to_lowercase().contains(&q)
                || slug.contains(q.as_str())
            {
                out.push(installed_to_dto(u, &agent_id));
            }
        }
    }
    Ok(out)
}

pub fn stats_summary(
    state: &AppState,
    scope: Scope,
    _project_root: Option<&Path>,
) -> Result<StatsDto, String> {
    let mut agents_out: Vec<AgentStatsDto> = Vec::new();
    let mut grand_count: u64 = 0;
    let mut grand_bytes: u64 = 0;

    for a in state.agents() {
        let agent_id = a.agent().as_str().to_owned();
        let units = a.list(scope).unwrap_or_default();
        let mut by_type: BTreeMap<&'static str, (u64, u64)> = BTreeMap::new();
        let mut total_count: u64 = 0;
        let mut total_bytes: u64 = 0;
        for u in &units {
            let bytes: u64 = u
                .paths
                .iter()
                .filter_map(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
                .sum();
            let slot = by_type
                .entry(unit_type_slug(u.unit_ref.unit_type))
                .or_insert((0, 0));
            slot.0 += 1;
            slot.1 += bytes;
            total_count += 1;
            total_bytes += bytes;
        }
        let by_type_vec: Vec<TypeStatsDto> = by_type
            .into_iter()
            .map(|(ty, (c, b))| TypeStatsDto {
                unit_type: ty.to_owned(),
                count: c,
                bytes: b,
            })
            .collect();
        grand_count += total_count;
        grand_bytes += total_bytes;
        agents_out.push(AgentStatsDto {
            agent: agent_id,
            by_type: by_type_vec,
            total_count,
            total_bytes,
        });
    }

    Ok(StatsDto {
        agents: agents_out,
        grand_total_count: grand_count,
        grand_total_bytes: grand_bytes,
    })
}

pub fn doctor_scan(
    state: &AppState,
    scope: Scope,
    project_root: Option<&Path>,
    fix: bool,
) -> Result<DoctorResultDto, String> {
    // Collect all units at this scope across both adapters.
    let mut all: Vec<(AgentId, Scope, InstalledUnit)> = Vec::new();
    for a in state.agents() {
        let agent = a.agent();
        if let Ok(units) = a.list(scope) {
            for u in units {
                all.push((agent.clone(), scope, u));
            }
        }
    }

    // Duplicates: same (type,name) on 2+ agents.
    type DupEntry = (String, Scope, PathBuf);
    let mut by_key: HashMap<(UnitType, String), Vec<DupEntry>> = HashMap::new();
    for (agent, sc, u) in &all {
        let first = u.paths.first().cloned().unwrap_or_default();
        by_key
            .entry((u.unit_ref.unit_type, u.unit_ref.name.clone()))
            .or_default()
            .push((agent.as_str().to_owned(), *sc, first));
    }
    let mut duplicates: Vec<DuplicateDto> = Vec::new();
    for ((ty, name), entries) in &by_key {
        let distinct: BTreeSet<_> = entries.iter().map(|(a, _, _)| a.as_str()).collect();
        if distinct.len() >= 2 {
            duplicates.push(DuplicateDto {
                unit_type: unit_type_slug(*ty).to_owned(),
                name: name.clone(),
                locations: entries
                    .iter()
                    .map(|(ag, sc, p)| DuplicateLocationDto {
                        agent: ag.clone(),
                        scope: *sc,
                        path: p.clone(),
                    })
                    .collect(),
            });
        }
    }

    // Broken symlinks.
    let mut broken_symlinks: Vec<String> = Vec::new();
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
                                broken_symlinks.push(format!(
                                    "{} -> {}",
                                    p.display(),
                                    target.display()
                                ));
                            }
                        }
                        Err(_) => broken_symlinks.push(format!("{} -> ?", p.display())),
                    }
                }
            }
        }
    }

    // Mv reconciliation — only considers the given scope (GUI view is
    // scope-scoped). Compare disk-present vs lockfile-claimed.
    type Triple = (String, UnitType, String);
    let mut disk_scopes: HashMap<Triple, HashSet<Scope>> = HashMap::new();
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
    let mut lock_scopes: HashMap<Triple, HashSet<Scope>> = HashMap::new();
    let prev_lock = store::load_lockfile(scope, project_root).map_err(|e| e.to_string())?;
    for e in &prev_lock.entries {
        if e.scope != scope {
            continue;
        }
        let name = lock_entry_name(e);
        lock_scopes
            .entry((e.agent.as_str().to_owned(), e.unit_type, name))
            .or_default()
            .insert(e.scope);
    }

    let mut all_keys: BTreeSet<Triple> = BTreeSet::new();
    for k in disk_scopes.keys() {
        all_keys.insert(k.clone());
    }
    for k in lock_scopes.keys() {
        all_keys.insert(k.clone());
    }

    let mut mv_split: Vec<String> = Vec::new();
    let mut mv_stale_lock: Vec<String> = Vec::new();
    let mut fixed: u32 = 0;

    for key in &all_keys {
        let disk = disk_scopes.get(key).cloned().unwrap_or_default();
        let lock = lock_scopes.get(key).cloned().unwrap_or_default();
        if disk == lock {
            continue;
        }
        let only_disk: Vec<_> = disk.difference(&lock).copied().collect();
        let only_lock: Vec<_> = lock.difference(&disk).copied().collect();

        if !only_disk.is_empty() {
            mv_split.push(format!(
                "[{}] {}/{} on disk but not in lockfile",
                key.0,
                unit_type_slug(key.1),
                key.2,
            ));
        }
        if !only_lock.is_empty() {
            mv_stale_lock.push(format!(
                "[{}] {}/{} in lockfile but not installed",
                key.0,
                unit_type_slug(key.1),
                key.2,
            ));
            if fix {
                let mut l = store::load_lockfile(scope, project_root).map_err(|e| e.to_string())?;
                let before = l.entries.len();
                l.entries.retain(|e| {
                    !(e.agent.as_str() == key.0
                        && e.unit_type == key.1
                        && lock_entry_name(e) == key.2
                        && e.scope == scope)
                });
                if l.entries.len() != before {
                    store::save_lockfile(scope, project_root, &l).map_err(|e| e.to_string())?;
                    fixed += 1;
                }
            }
        }
    }

    Ok(DoctorResultDto {
        duplicates,
        broken_symlinks,
        mv_split,
        mv_stale_lock,
        fixed,
    })
}

/// Same as CLI lock_entry_name — prefer native_name, then parent dir
/// for skill layouts, then file stem.
fn lock_entry_name(e: &LockEntry) -> String {
    if let Some(n) = &e.native_name {
        return n.clone();
    }
    if let Some(stem) = e.path.file_stem().and_then(|s| s.to_str()) {
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
