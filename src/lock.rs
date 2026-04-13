use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub skills: HashMap<String, LockEntry>,
}

impl Default for LockFile {
    fn default() -> Self {
        Self { version: 1, skills: HashMap::new() }
    }
}

/// One installed skill tracked in the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    /// Source identifier, e.g. "github:anthropics/skills"
    pub source: String,
    /// Full 40-char Git commit hash at install time
    pub commit: String,
    /// Git ref (branch / tag) used, None → default branch
    #[serde(default)]
    pub git_ref: Option<String>,
    /// Subpath within the repo for multi-skill repos
    #[serde(default)]
    pub subpath: Option<String>,
    /// Unix timestamp (seconds) of install
    pub installed_at: u64,
}

// ── Path ────────────────────────────────────────────────────────────────────

pub fn lock_path() -> PathBuf {
    crate::store::home().join(".rig/skill-lock.json")
}

// ── I/O ─────────────────────────────────────────────────────────────────────

pub fn read() -> LockFile {
    let path = lock_path();
    if !path.exists() {
        return LockFile::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return LockFile::default(),
    };
    match serde_json::from_str::<LockFile>(&content) {
        Ok(lock) => lock,
        Err(_) => {
            // Back up the corrupted file before resetting
            let backup = path.with_extension("json.bak");
            let _ = std::fs::copy(&path, &backup);
            eprintln!(
                "Warning: skill-lock.json was corrupted. Backed up to {}. Starting fresh.",
                backup.display()
            );
            LockFile::default()
        }
    }
}

pub fn write(lock: &LockFile) -> Result<()> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(lock)? + "\n")?;
    Ok(())
}

/// Insert or update a single skill entry.
pub fn upsert(name: &str, entry: LockEntry) -> Result<()> {
    let mut lock = read();
    lock.skills.insert(name.to_string(), entry);
    write(&lock)
}

/// Remove a skill entry (no-op if not present).
pub fn remove(name: &str) -> Result<()> {
    let mut lock = read();
    if lock.skills.remove(name).is_some() {
        write(&lock)?;
    }
    Ok(())
}

/// Current Unix timestamp in seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
