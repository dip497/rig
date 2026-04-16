//! `.rig/` on-disk state: manifest + lockfile at the scope root.
//!
//! - `project` scope → `./.rig/rig.toml` + `./.rig/rig.lock`
//! - `global`  scope → `~/.rig/rig.toml` + `~/.rig/rig.lock`

use std::path::PathBuf;

use anyhow::{Context, Result};

use rig_core::lockfile::Lockfile;
use rig_core::manifest::{Manifest, SCHEMA};
use rig_core::scope::Scope;

pub fn scope_dir(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => {
            let home = rig_fs::home_dir().context("discovering home dir")?;
            Ok(home.join(".rig"))
        }
        Scope::Project => Ok(PathBuf::from(".rig")),
    }
}

pub fn manifest_path(scope: Scope) -> Result<PathBuf> {
    Ok(scope_dir(scope)?.join("rig.toml"))
}

pub fn lockfile_path(scope: Scope) -> Result<PathBuf> {
    Ok(scope_dir(scope)?.join("rig.lock"))
}

/// Load the manifest, or return an empty one if the file doesn't exist.
pub fn load_manifest(scope: Scope) -> Result<Manifest> {
    let p = manifest_path(scope)?;
    if !p.exists() {
        return Ok(empty_manifest());
    }
    let bytes = rig_fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
    let s = std::str::from_utf8(&bytes).with_context(|| format!("{} is not UTF-8", p.display()))?;
    Manifest::parse(s).with_context(|| format!("parsing {}", p.display()))
}

pub fn save_manifest(scope: Scope, manifest: &Manifest) -> Result<()> {
    let p = manifest_path(scope)?;
    let s = toml::to_string_pretty(manifest).context("serialising manifest")?;
    rig_fs::atomic_write(&p, s.as_bytes()).with_context(|| format!("writing {}", p.display()))?;
    Ok(())
}

pub fn load_lockfile(scope: Scope) -> Result<Lockfile> {
    let p = lockfile_path(scope)?;
    if !p.exists() {
        return Ok(Lockfile::new());
    }
    let bytes = rig_fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
    let s = std::str::from_utf8(&bytes).with_context(|| format!("{} is not UTF-8", p.display()))?;
    Lockfile::parse(s).with_context(|| format!("parsing {}", p.display()))
}

pub fn save_lockfile(scope: Scope, lock: &Lockfile) -> Result<()> {
    let p = lockfile_path(scope)?;
    let s = lock.to_toml().context("serialising lockfile")?;
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
