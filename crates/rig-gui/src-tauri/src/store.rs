//! `.rig/` on-disk state: manifest + lockfile at the scope root.
//!
//! Adapted from `rig-cli/src/store.rs` — this version accepts an
//! explicit project root path instead of relying on the process cwd,
//! so the GUI can switch between projects without chdir.
//!
//! - `project` scope → `<project_root>/.rig/rig.toml` + `rig.lock`
//! - `global`  scope → `~/.rig/rig.toml` + `rig.lock`

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use rig_core::lockfile::Lockfile;
use rig_core::manifest::{Manifest, SCHEMA};
use rig_core::scope::Scope;

pub fn scope_dir(scope: Scope, project_root: Option<&Path>) -> Result<PathBuf> {
    match scope {
        Scope::Global => {
            let home = rig_fs::home_dir().context("discovering home dir")?;
            Ok(home.join(".rig"))
        }
        Scope::Project => {
            let root = project_root
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            Ok(root.join(".rig"))
        }
    }
}

pub fn manifest_path(scope: Scope, project_root: Option<&Path>) -> Result<PathBuf> {
    Ok(scope_dir(scope, project_root)?.join("rig.toml"))
}

pub fn lockfile_path(scope: Scope, project_root: Option<&Path>) -> Result<PathBuf> {
    Ok(scope_dir(scope, project_root)?.join("rig.lock"))
}

/// Load the manifest, or return an empty one if the file doesn't exist.
pub fn load_manifest(scope: Scope, project_root: Option<&Path>) -> Result<Manifest> {
    let p = manifest_path(scope, project_root)?;
    if !p.exists() {
        return Ok(empty_manifest());
    }
    let bytes = rig_fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
    let s = std::str::from_utf8(&bytes).with_context(|| format!("{} is not UTF-8", p.display()))?;
    Manifest::parse(s).with_context(|| format!("parsing {}", p.display()))
}

pub fn load_lockfile(scope: Scope, project_root: Option<&Path>) -> Result<Lockfile> {
    let p = lockfile_path(scope, project_root)?;
    if !p.exists() {
        return Ok(Lockfile::new());
    }
    let bytes = rig_fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
    let s = std::str::from_utf8(&bytes).with_context(|| format!("{} is not UTF-8", p.display()))?;
    Lockfile::parse(s).with_context(|| format!("parsing {}", p.display()))
}

pub fn save_manifest(scope: Scope, project_root: Option<&Path>, manifest: &Manifest) -> Result<()> {
    let p = manifest_path(scope, project_root)?;
    let toml_str =
        toml::to_string_pretty(manifest).with_context(|| "serializing manifest to TOML")?;
    rig_fs::atomic_write(&p, toml_str.as_bytes())
        .with_context(|| format!("writing {}", p.display()))?;
    Ok(())
}

pub fn save_lockfile(scope: Scope, project_root: Option<&Path>, lockfile: &Lockfile) -> Result<()> {
    let p = lockfile_path(scope, project_root)?;
    let s = lockfile.to_toml().with_context(|| "serializing lockfile")?;
    rig_fs::atomic_write(&p, s.as_bytes()).with_context(|| format!("writing {}", p.display()))?;
    Ok(())
}

pub fn empty_manifest() -> Manifest {
    Manifest {
        schema: SCHEMA.to_owned(),
        project: None,
        agents: Default::default(),
        scope: Default::default(),
        bundles: Default::default(),
    }
}
