//! Tauri command surface for the Rig GUI (M1 read-only dashboard).
//!
//! All commands are thin wrappers over the adapter trait plus the
//! local manifest/lockfile store. No install/uninstall/sync flows live
//! here — CLI remains the source of truth for mutating operations in M1.

pub mod dto;
pub mod state;
pub mod store;

use std::path::{Path, PathBuf};

use rig_core::adapter::{Adapter, UnitRef};
use rig_core::scope::Scope;
use rig_core::unit::{Unit, UnitType};
use tauri::State;

use crate::dto::{
    unit_type_slug, AgentDto, DriftReportDto, InstalledUnitDto, LockfileDto, ManifestDto, ScopeDto,
    ScopeRootsDto, UnitBodyDto, UnitTypeDto,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rig GUI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_home() -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        tmp
    }

    #[test]
    fn list_agents_returns_two_first_party() {
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
        let _h = setup_home();
        let roots = scope_roots().unwrap();
        assert!(roots.global_rig.ends_with(".rig"));
        assert!(roots.claude_global.ends_with(".claude"));
        assert!(roots.codex_global.ends_with(".codex"));
    }

    #[test]
    fn manifest_roundtrip_via_store() {
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
