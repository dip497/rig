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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a temporary dir for lock file tests. Auto-cleaned on drop.
    struct TempHome {
        orig: PathBuf,
        tmp: PathBuf,
    }

    impl TempHome {
        fn new() -> Self {
            let tmp = std::env::temp_dir().join(format!("rig-test-lock-{}", std::process::id()));
            let _ = fs::create_dir_all(&tmp);
            // Override lock_path by setting a temp home
            Self {
                orig: dirs::home_dir().unwrap_or_default(),
                tmp,
            }
        }

        fn lock_path(&self) -> PathBuf {
            self.tmp.join("skill-lock.json")
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.tmp);
        }
    }

    fn sample_entry() -> LockEntry {
        LockEntry {
            source: "github:owner/repo".into(),
            commit: "abc123def456abc123def456abc123def456abcd".into(),
            git_ref: Some("main".into()),
            subpath: Some("skills/my-skill".into()),
            installed_at: 1700000000,
        }
    }

    #[test]
    fn test_lock_default_is_empty() {
        let lock = LockFile::default();
        assert_eq!(lock.version, 1);
        assert!(lock.skills.is_empty());
    }

    #[test]
    fn test_lock_roundtrip_json() {
        let mut lock = LockFile::default();
        lock.skills.insert("my-skill".into(), sample_entry());

        let json = serde_json::to_string_pretty(&lock).unwrap();
        let parsed: LockFile = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.skills.len(), 1);
        let entry = parsed.skills.get("my-skill").unwrap();
        assert_eq!(entry.source, "github:owner/repo");
        assert_eq!(entry.commit, "abc123def456abc123def456abc123def456abcd");
        assert_eq!(entry.git_ref.as_deref(), Some("main"));
        assert_eq!(entry.subpath.as_deref(), Some("skills/my-skill"));
    }

    #[test]
    fn test_lock_multiple_entries() {
        let mut lock = LockFile::default();
        lock.skills.insert("skill-a".into(), LockEntry {
            source: "github:a/repo".into(),
            commit: "a".repeat(40),
            git_ref: None,
            subpath: None,
            installed_at: 100,
        });
        lock.skills.insert("skill-b".into(), LockEntry {
            source: "github:b/repo".into(),
            commit: "b".repeat(40),
            git_ref: Some("v2".into()),
            subpath: None,
            installed_at: 200,
        });

        let json = serde_json::to_string(&lock).unwrap();
        let parsed: LockFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.skills.len(), 2);
        assert!(parsed.skills.contains_key("skill-a"));
        assert!(parsed.skills.contains_key("skill-b"));
    }

    #[test]
    fn test_lock_deserializes_missing_optional_fields() {
        // git_ref and subpath are #[serde(default)] — should deserialize from JSON without them
        let json = r#"{"version":1,"skills":{"test":{"source":"local:x","commit":"abc","installed_at":5}}}"#;
        let lock: LockFile = serde_json::from_str(json).unwrap();
        let entry = lock.skills.get("test").unwrap();
        assert!(entry.git_ref.is_none());
        assert!(entry.subpath.is_none());
    }

    #[test]
    fn test_now_returns_reasonable_timestamp() {
        let t = now();
        // Should be after 2020-01-01 and before 2030-01-01
        assert!(t > 1577836800, "timestamp too old: {t}");
        assert!(t < 1893456000, "timestamp too new: {t}");
    }
}
