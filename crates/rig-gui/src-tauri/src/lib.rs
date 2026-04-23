//! Tauri command surface for the Rig GUI (M1 read-only dashboard).
//!
//! All commands are thin wrappers over the adapter trait plus the
//! local manifest/lockfile store. No install/uninstall/sync flows live
//! here — CLI remains the source of truth for mutating operations in M1.

pub mod dto;
pub mod state;
pub mod store;

use std::path::{Path, PathBuf};

use rig_adapter_claude::{CommandConverter, RuleConverter, SkillConverter, SubagentConverter};
use rig_core::adapter::{Adapter, UnitRef};
use rig_core::agent::AgentId;
use rig_core::converter::Converter;
use rig_core::lockfile::LockEntry;
use rig_core::scope::Scope;
use rig_core::source::Source;
use rig_core::unit::{Unit, UnitType};
use tauri::State;

use crate::dto::{
    unit_type_slug, AgentDto, DriftReportDto, InstallResultDto, InstalledUnitDto, LockfileDto,
    ManifestDto, MvResultDto, ScopeDto, ScopeRootsDto, UnitBodyDto, UnitTypeDto,
};
use crate::state::AppState;

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn project_root(project_path: Option<String>) -> Option<PathBuf> {
    project_path.map(PathBuf::from)
}

fn installed_to_dto(iu: rig_core::adapter::InstalledUnit, agent: &str) -> InstalledUnitDto {
    InstalledUnitDto {
        agent: agent.to_owned(),
        unit_type: unit_type_slug(iu.unit_ref.unit_type).to_owned(),
        name: iu.unit_ref.name,
        paths: iu.paths,
        disabled: iu.disabled,
    }
}

#[tauri::command]
fn list_agents(state: State<'_, AppState>) -> Vec<AgentDto> {
    state
        .agents()
        .iter()
        .map(|a| AgentDto {
            id: a.agent().as_str().to_owned(),
            capabilities: a
                .capabilities()
                .iter()
                .map(|t| unit_type_slug(*t).to_owned())
                .collect(),
        })
        .collect()
}

#[tauri::command]
fn list_units(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
) -> Result<Vec<InstalledUnitDto>, String> {
    // project_path is accepted for future per-project isolation;
    // adapters still use global home + process cwd internally.
    let _ = project_root(project_path);
    let mut out = Vec::new();
    for a in state.agents() {
        let agent_id = a.agent().as_str().to_owned();
        let units = a.list(scope).map_err(map_err)?;
        out.extend(units.into_iter().map(|u| installed_to_dto(u, &agent_id)));
    }
    Ok(out)
}

#[tauri::command]
fn detect_drift(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
) -> Result<DriftReportDto, String> {
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;

    let root = project_root(project_path);
    let lock = store::load_lockfile(scope, root.as_deref()).map_err(map_err)?;
    let agent_id = rig_core::agent::AgentId::new(&agent);

    let entry = lock.entries.iter().find(|e| {
        e.unit_type == unit_type && e.agent == agent_id && e.scope == scope && e.id.ends_with(&name)
    });

    let unit_ref = UnitRef::new(unit_type, name.clone());
    let install_sha = entry
        .map(|e| e.install_sha.clone())
        .unwrap_or_else(|| rig_core::source::Sha256::of(b""));

    let (drift_state, shas) = adapter
        .detect_drift(&unit_ref, scope, install_sha.clone(), None)
        .map_err(map_err)?;

    Ok(DriftReportDto {
        state: drift_state,
        install_sha: entry.map(|e| e.install_sha.as_str().to_owned()),
        current_sha: shas.current_disk.as_ref().map(|s| s.as_str().to_owned()),
        upstream_sha: shas.upstream.as_ref().map(|s| s.as_str().to_owned()),
    })
}

fn extract_body(unit: &Unit) -> (String, String, PathBuf) {
    // Best-effort preview: body + a frontmatter-ish YAML block where
    // applicable. Path left empty when adapter doesn't surface it.
    match unit {
        Unit::Skill(s) => {
            let fm = format!("name: {}\ndescription: {}\n", s.name, s.description);
            (s.body.clone(), fm, PathBuf::new())
        }
        Unit::Rule(r) => {
            let fm = format!(
                "name: {}\ndescription: {}\nplacement: {:?}\n",
                r.name,
                r.description.clone().unwrap_or_default(),
                r.placement
            );
            (r.body.clone(), fm, PathBuf::new())
        }
        Unit::Command(c) => {
            let body = format!("{c:#?}");
            (body, String::new(), PathBuf::new())
        }
        Unit::Subagent(s) => {
            let body = format!("{s:#?}");
            (body, String::new(), PathBuf::new())
        }
        other => (format!("{other:#?}"), String::new(), PathBuf::new()),
    }
}

#[tauri::command]
fn read_unit_body(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
) -> Result<UnitBodyDto, String> {
    let _ = project_root(project_path);
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;
    let unit = adapter
        .read_local(&UnitRef::new(unit_type, name), scope)
        .map_err(map_err)?;
    let (body, frontmatter, path) = extract_body(&unit);
    Ok(UnitBodyDto {
        body,
        frontmatter,
        path,
    })
}

#[tauri::command]
fn read_manifest(scope: ScopeDto, project_path: Option<String>) -> Result<ManifestDto, String> {
    let root = project_root(project_path);
    let path = store::manifest_path(scope, root.as_deref()).map_err(map_err)?;
    let exists = path.exists();
    let manifest = store::load_manifest(scope, root.as_deref()).map_err(map_err)?;
    Ok(ManifestDto {
        manifest,
        path,
        exists,
    })
}

#[tauri::command]
fn read_lockfile(scope: ScopeDto, project_path: Option<String>) -> Result<LockfileDto, String> {
    let root = project_root(project_path);
    let path = store::lockfile_path(scope, root.as_deref()).map_err(map_err)?;
    let exists = path.exists();
    let lockfile = store::load_lockfile(scope, root.as_deref()).map_err(map_err)?;
    Ok(LockfileDto {
        lockfile,
        path,
        exists,
    })
}

#[tauri::command]
fn scope_roots() -> Result<ScopeRootsDto, String> {
    let home = rig_fs::home_dir().map_err(map_err)?;
    Ok(ScopeRootsDto {
        global_rig: home.join(".rig"),
        claude_global: home.join(".claude"),
        codex_global: home.join(".codex"),
        home,
    })
}

fn parse_unit_from_native(
    unit_type: UnitType,
    native: &rig_core::converter::NativeLayout,
) -> Result<Unit, String> {
    Ok(match unit_type {
        UnitType::Skill => Unit::Skill(SkillConverter.parse_native(native).map_err(map_err)?),
        UnitType::Rule => Unit::Rule(RuleConverter.parse_native(native).map_err(map_err)?),
        UnitType::Command => Unit::Command(CommandConverter.parse_native(native).map_err(map_err)?),
        UnitType::Subagent => {
            Unit::Subagent(SubagentConverter.parse_native(native).map_err(map_err)?)
        }
        other => return Err(format!("unit type `{other:?}` not yet supported by GUI")),
    })
}

#[tauri::command]
fn install_unit(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    source: String,
    as_type: Option<UnitTypeDto>,
    agents: Vec<String>,
) -> Result<InstallResultDto, String> {
    let parsed = Source::parse(&source).map_err(map_err)?;
    let fetched = rig_source::fetch(&parsed).map_err(map_err)?;
    let unit_type = match (fetched.detected, as_type) {
        (_, Some(t)) => t,
        (Some(t), None) => t,
        (None, None) => {
            return Err("could not auto-detect unit type; pass as_type".into());
        }
    };
    let unit = parse_unit_from_native(unit_type, &fetched.native)?;
    let source_sha = fetched.source_sha.as_str().to_owned();

    let root = project_root(project_path);
    let mut lock = store::load_lockfile(scope, root.as_deref()).map_err(map_err)?;
    let mut installed: Vec<InstalledUnitDto> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for agent_id in &agents {
        let Some(adapter) = state.adapter_by_id(agent_id) else {
            skipped.push(format!("unknown agent `{agent_id}`"));
            continue;
        };
        if !adapter.capabilities().contains(&unit_type) {
            skipped.push(format!(
                "{agent_id} does not support {}",
                unit_type_slug(unit_type)
            ));
            continue;
        }
        let receipt = adapter.install(&unit, scope).map_err(map_err)?;
        let id = format!("{}/{}", unit_type_slug(unit_type), parsed);

        lock.entries
            .retain(|e| !(e.id == id && e.agent == receipt.agent && e.scope == scope));
        let native_name = if unit_type == rig_core::unit::UnitType::Mcp {
            Some(receipt.unit_ref.name.clone())
        } else {
            None
        };
        lock.entries.push(LockEntry {
            id,
            unit_type,
            source: parsed.clone(),
            source_sha: fetched.source_sha.clone(),
            install_sha: receipt.install_sha.clone(),
            agent: receipt.agent.clone(),
            scope,
            path: receipt.paths.first().cloned().unwrap_or_else(PathBuf::new),
            native_name,
            extra: Default::default(),
        });
        installed.push(InstalledUnitDto {
            agent: agent_id.clone(),
            unit_type: unit_type_slug(unit_type).to_owned(),
            name: receipt.unit_ref.name.clone(),
            paths: receipt.paths,
            disabled: false,
        });
    }

    if !installed.is_empty() {
        store::save_lockfile(scope, root.as_deref(), &lock).map_err(map_err)?;
    }

    Ok(InstallResultDto {
        installed,
        skipped,
        source_sha,
    })
}

#[tauri::command]
fn uninstall_unit(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
) -> Result<(), String> {
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;
    if !adapter.capabilities().contains(&unit_type) {
        return Err(format!(
            "{agent} does not support {}",
            unit_type_slug(unit_type)
        ));
    }
    adapter
        .uninstall(&UnitRef::new(unit_type, name.clone()), scope)
        .map_err(map_err)?;

    let root = project_root(project_path);
    let agent_id = AgentId::new(&agent);
    let mut lock = store::load_lockfile(scope, root.as_deref()).map_err(map_err)?;
    let before = lock.entries.len();
    lock.entries.retain(|e| {
        if e.unit_type != unit_type || e.agent != agent_id || e.scope != scope {
            return true;
        }
        // Skill layout: <type_root>/<name>/SKILL.md → match parent dir name.
        // Single-file layout: <type_root>/<name>.md → match file_stem.
        let matches_dir = e
            .path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            == Some(name.as_str());
        let matches_stem = e.path.file_stem().and_then(|s| s.to_str()) == Some(name.as_str());
        !(matches_dir || matches_stem)
    });
    if lock.entries.len() != before {
        store::save_lockfile(scope, root.as_deref(), &lock).map_err(map_err)?;
    }
    Ok(())
}

#[tauri::command]
fn set_enabled(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    let _ = project_root(project_path);
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;
    adapter
        .set_enabled(&UnitRef::new(unit_type, name), scope, enabled)
        .map_err(map_err)
}

#[tauri::command]
fn is_enabled(
    state: State<'_, AppState>,
    scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
) -> Result<bool, String> {
    let _ = project_root(project_path);
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;
    adapter
        .is_enabled(&UnitRef::new(unit_type, name), scope)
        .map_err(map_err)
}

/// Per-(agent, unit) scope move. Mirrors the CLI `rig mv` 5-step flow
/// in crates/rig-cli/src/main.rs::mv but only for a single adapter and
/// without the multi-agent pre-flight. Steps:
///   1. Read source lockfile + `read_local` the unit.
///   2. Query current `is_enabled(from)` to preserve disabled state.
///   3. `install` into target scope.
///   4. Append/replace target lockfile entry (carrying source/source_sha).
///   5. Re-apply disabled state at target if needed.
///   6. `uninstall` from source.
///   7. Drop source lockfile entry.
#[tauri::command]
fn mv_unit(
    state: State<'_, AppState>,
    from_scope: ScopeDto,
    to_scope: ScopeDto,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitTypeDto,
    name: String,
) -> Result<MvResultDto, String> {
    mv_unit_inner(
        &state,
        from_scope,
        to_scope,
        project_path,
        agent,
        unit_type,
        name,
    )
}

fn mv_unit_inner(
    state: &AppState,
    from_scope: Scope,
    to_scope: Scope,
    project_path: Option<String>,
    agent: String,
    unit_type: UnitType,
    name: String,
) -> Result<MvResultDto, String> {
    let adapter = state
        .adapter_by_id(&agent)
        .ok_or_else(|| format!("unknown agent `{agent}`"))?;
    if !adapter.capabilities().contains(&unit_type) {
        return Err(format!(
            "{agent} does not support {}",
            unit_type_slug(unit_type)
        ));
    }
    if from_scope == to_scope {
        return Err(format!(
            "from and to scope are identical ({})",
            match from_scope {
                Scope::Global => "global",
                Scope::Project => "project",
                Scope::Local => "local",
            }
        ));
    }

    let root = project_root(project_path);
    let unit_ref = UnitRef::new(unit_type, name.clone());
    let agent_id = AgentId::new(&agent);

    // Step 1: load source lock + read unit.
    let src_lock = store::load_lockfile(from_scope, root.as_deref()).map_err(map_err)?;
    let src_entry = src_lock
        .entries
        .iter()
        .find(|e| {
            e.unit_type == unit_type
                && e.agent == agent_id
                && e.scope == from_scope
                && lock_entry_name(e) == name
        })
        .cloned();

    let unit = adapter.read_local(&unit_ref, from_scope).map_err(map_err)?;

    // Step 2: preserve disabled state.
    let was_disabled = adapter
        .is_enabled(&unit_ref, from_scope)
        .map(|e| !e)
        .unwrap_or(false);

    // Step 3: install into target.
    let receipt = adapter.install(&unit, to_scope).map_err(map_err)?;

    // Step 4: update target lockfile.
    let id = src_entry
        .as_ref()
        .map(|e| e.id.clone())
        .unwrap_or_else(|| format!("{}/{}", unit_type_slug(unit_type), name));
    let source = src_entry
        .as_ref()
        .map(|e| e.source.clone())
        .unwrap_or_else(|| Source::Local { path: name.clone() });
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

    let mut target_lock = store::load_lockfile(to_scope, root.as_deref()).map_err(map_err)?;
    target_lock
        .entries
        .retain(|e| !(e.id == id && e.agent == receipt.agent && e.scope == to_scope));
    target_lock.entries.push(LockEntry {
        id: id.clone(),
        unit_type,
        source,
        source_sha,
        install_sha: receipt.install_sha.clone(),
        agent: receipt.agent.clone(),
        scope: to_scope,
        path: receipt.paths.first().cloned().unwrap_or_default(),
        native_name,
        extra,
    });
    store::save_lockfile(to_scope, root.as_deref(), &target_lock).map_err(map_err)?;

    // Step 5: re-apply disabled state at target.
    if was_disabled {
        let _ = adapter.set_enabled(&unit_ref, to_scope, false);
    }

    // Step 6: uninstall source.
    adapter.uninstall(&unit_ref, from_scope).map_err(map_err)?;

    // Step 7: drop source lock entry.
    let mut src_lock_mut = src_lock;
    src_lock_mut
        .entries
        .retain(|e| !(e.id == id && e.agent == agent_id && e.scope == from_scope));
    store::save_lockfile(from_scope, root.as_deref(), &src_lock_mut).map_err(map_err)?;

    Ok(MvResultDto {
        from_scope,
        to_scope,
        install_sha: receipt.install_sha.as_str().to_owned(),
        disabled: was_disabled,
    })
}

/// Return the logical unit name for a lockfile entry, mirroring the
/// CLI's `lock_entry_name` — falls back to path basename/parent-name.
fn lock_entry_name(e: &LockEntry) -> String {
    if let Some(n) = &e.native_name {
        return n.clone();
    }
    // Skill layout: <root>/<name>/SKILL.md — prefer parent dir name.
    if let Some(parent) = e
        .path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
    {
        // Use parent name unless it's a generic root like "skills" etc.
        let stem = e.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if e.path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| n.eq_ignore_ascii_case("SKILL.md"))
            .unwrap_or(false)
        {
            return parent.to_owned();
        }
        if !stem.is_empty() {
            return stem.to_owned();
        }
        return parent.to_owned();
    }
    e.path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned()
}

// Silence unused-imports when no test feature picks them up.
#[allow(dead_code)]
fn _type_assertions(_: UnitType, _: Scope, _: &dyn Adapter, _: &Path) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            list_agents,
            list_units,
            detect_drift,
            read_unit_body,
            read_manifest,
            read_lockfile,
            scope_roots,
            install_unit,
            uninstall_unit,
            set_enabled,
            is_enabled,
            mv_unit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rig GUI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};
    use tempfile::TempDir;

    // Serialize any test that mutates process-global state ($HOME, cwd).
    // Mirrors the HOME_LOCK pattern in crates/rig-cli/src/main.rs.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    fn home_guard() -> MutexGuard<'static, ()> {
        HOME_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn setup_home() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        tmp
    }

    #[test]
    fn list_agents_returns_two_first_party() {
        let _g = home_guard();
        let _h = setup_home();
        let st = AppState::new();
        let agents: Vec<_> = st
            .agents()
            .iter()
            .map(|a| a.agent().as_str().to_owned())
            .collect();
        assert!(agents.contains(&"claude".to_string()));
        assert!(agents.contains(&"codex".to_string()));
    }

    #[test]
    fn list_units_empty_global_home() {
        let _g = home_guard();
        let _h = setup_home();
        let st = AppState::new();
        let mut all = Vec::new();
        for a in st.agents() {
            all.extend(a.list(Scope::Global).unwrap());
        }
        assert!(all.is_empty(), "expected empty, got {:?}", all);
    }

    #[test]
    fn read_manifest_missing_returns_empty() {
        let _g = home_guard();
        let home = setup_home();
        let tmp_proj = tempfile::tempdir().unwrap();
        let dto = read_manifest(
            Scope::Project,
            Some(tmp_proj.path().to_string_lossy().to_string()),
        )
        .unwrap();
        assert!(!dto.exists);
        assert_eq!(dto.manifest.schema, "rig/v1");
        let _ = home;
    }

    #[test]
    fn read_lockfile_missing_returns_empty() {
        let _g = home_guard();
        let _h = setup_home();
        let tmp_proj = tempfile::tempdir().unwrap();
        let dto = read_lockfile(
            Scope::Project,
            Some(tmp_proj.path().to_string_lossy().to_string()),
        )
        .unwrap();
        assert!(!dto.exists);
        assert!(dto.lockfile.entries.is_empty());
    }

    #[test]
    fn scope_roots_has_home() {
        let _g = home_guard();
        let _h = setup_home();
        let roots = scope_roots().unwrap();
        assert!(roots.global_rig.ends_with(".rig"));
        assert!(roots.claude_global.ends_with(".claude"));
        assert!(roots.codex_global.ends_with(".codex"));
    }

    fn write_skill(root: &Path, name: &str) {
        let dir = root.join(".claude/skills").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n\n# {name}\nbody\n"),
        )
        .unwrap();
    }

    #[test]
    fn set_and_is_enabled_roundtrip_for_skill() {
        let _g = home_guard();
        let home = setup_home();
        write_skill(home.path(), "demo-skill");

        let st = AppState::new();
        let adapter = st.adapter_by_id("claude").unwrap();
        let unit_ref = UnitRef::new(UnitType::Skill, "demo-skill".to_owned());

        // Initially enabled.
        assert!(adapter.is_enabled(&unit_ref, Scope::Global).unwrap());

        // Disable via trait, mirroring the set_enabled command body.
        adapter
            .set_enabled(&unit_ref, Scope::Global, false)
            .unwrap();
        assert!(!adapter.is_enabled(&unit_ref, Scope::Global).unwrap());

        // Re-enable.
        adapter.set_enabled(&unit_ref, Scope::Global, true).unwrap();
        assert!(adapter.is_enabled(&unit_ref, Scope::Global).unwrap());
    }

    #[test]
    fn mv_unit_moves_skill_global_to_project_and_preserves_disabled() {
        let _g = home_guard();
        let home = setup_home();
        write_skill(home.path(), "movable");

        let tmp_proj = tempfile::tempdir().unwrap();
        // ClaudeAdapter resolves project-scope paths via cwd.
        let prev_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp_proj.path()).unwrap();

        let st = AppState::new();
        let adapter = st.adapter_by_id("claude").unwrap();
        let unit_ref = UnitRef::new(UnitType::Skill, "movable".to_owned());

        // Disable at source first.
        adapter
            .set_enabled(&unit_ref, Scope::Global, false)
            .unwrap();
        assert!(!adapter.is_enabled(&unit_ref, Scope::Global).unwrap());

        let res = mv_unit_inner(
            &st,
            Scope::Global,
            Scope::Project,
            Some(tmp_proj.path().to_string_lossy().to_string()),
            "claude".to_owned(),
            UnitType::Skill,
            "movable".to_owned(),
        );

        // Always restore cwd before any assertion that might panic.
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                let _ = std::env::set_current_dir(&prev_cwd);
                panic!("mv_unit_inner failed: {e}");
            }
        };
        assert_eq!(res.from_scope, Scope::Global);
        assert_eq!(res.to_scope, Scope::Project);
        assert!(res.disabled, "disabled flag must round-trip across mv");

        let src_ok = adapter.read_local(&unit_ref, Scope::Global).is_err();
        let target_disabled = !adapter.is_enabled(&unit_ref, Scope::Project).unwrap();
        let _ = std::env::set_current_dir(&prev_cwd);

        assert!(src_ok, "source should be gone");
        assert!(target_disabled, "disabled state lost across mv");
    }

    #[test]
    fn manifest_roundtrip_via_store() {
        let _g = home_guard();
        let _h = setup_home();
        let tmp_proj = tempfile::tempdir().unwrap();
        let rig_dir = tmp_proj.path().join(".rig");
        fs::create_dir_all(&rig_dir).unwrap();
        fs::write(
            rig_dir.join("rig.toml"),
            "schema = \"rig/v1\"\n[agents]\ntargets = [\"claude\"]\n",
        )
        .unwrap();
        let dto = read_manifest(
            Scope::Project,
            Some(tmp_proj.path().to_string_lossy().to_string()),
        )
        .unwrap();
        assert!(dto.exists);
        assert_eq!(dto.manifest.agents.targets.len(), 1);
    }
}
