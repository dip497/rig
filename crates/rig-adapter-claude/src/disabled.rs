//! Enable / disable mechanics for the Claude adapter.
//!
//! See `docs/ENABLE-DISABLE-MV.md` §§3-5.
//!
//! Per unit type:
//! - Skill → frontmatter flip (`disable-model-invocation: true` +
//!   `rig-disabled-at: <iso8601>` sentinel). Atomic rewrite.
//! - Rule / Command / Subagent → rename `<name>.md` ↔
//!   `<name>.md.rig-disabled`.
//! - Mcp → snapshot to `<scope>/.rig/disabled/mcp/<name>.claude.json`
//!   then `claude mcp remove`; reverse on enable.
//! - Hook / Plugin → `UnsupportedOp`.

use std::path::{Path, PathBuf};

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::scope::Scope;
use rig_core::unit::UnitType;

use crate::frontmatter;
use crate::{mcp, primary_path, scope_root, skill};

/// File-rename suffix used to hide a unit from Claude's loader.
pub const DISABLED_SUFFIX: &str = ".rig-disabled";

/// Frontmatter key Claude honours natively.
pub const KEY_DISABLE_INVOCATION: &str = "disable-model-invocation";
/// Rig-owned sentinel so `detect_drift` can recognise its own edit.
pub const KEY_RIG_DISABLED_AT: &str = "rig-disabled-at";

/// Compute `<scope>/.rig/disabled/mcp/<name>.claude.json` for an MCP.
/// Lives alongside the lockfile under `<scope>/.rig/`.
pub fn mcp_snapshot_path(scope: Scope, name: &str) -> AdapterResult<PathBuf> {
    let base = match scope {
        Scope::Global => {
            rig_fs::home_dir()
                .map(|h| h.join(".rig"))
                .map_err(|e| AdapterError::Other {
                    message: e.to_string(),
                    source: Some(Box::new(e)),
                })?
        }
        Scope::Project | Scope::Local => PathBuf::from(".rig"),
    };
    Ok(base
        .join("disabled")
        .join("mcp")
        .join(format!("{name}.claude.json")))
}

/// Current UTC timestamp in ISO 8601 format (e.g. `2026-04-21T10:14:22Z`).
/// Used as the value of the `rig-disabled-at` sentinel.
#[must_use]
pub fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_unix_secs(secs)
}

/// Turn a Unix timestamp in seconds into a naive UTC ISO-8601 string.
/// Pure so tests can pin a specific value.
#[must_use]
pub fn format_unix_secs(secs: i64) -> String {
    // Minimal gregorian conversion — avoids a chrono / time dep.
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400) as u32;
    let (y, m, d) = days_to_ymd(days);
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

// Civil-days-from-epoch → (y, m, d). Howard Hinnant's algorithm.
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

// ---------------- skill frontmatter normalisation ----------------

/// Strip `disable-model-invocation` and every `rig-disabled-*` key
/// from a SKILL.md document. Returns the unchanged bytes if the file
/// has no frontmatter fence, preserving pre-existing behaviour.
/// Pure — drives both disable/enable (for round-trip) and drift
/// hashing.
#[must_use]
pub fn normalise_skill_md(bytes: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let Some((fm_block, body)) = frontmatter::split(text) else {
        return bytes.to_vec();
    };
    let pairs = frontmatter::parse_flat(fm_block);
    let keep: Vec<(&str, &str)> = pairs
        .iter()
        .filter(|(k, _)| k != KEY_DISABLE_INVOCATION && !k.starts_with("rig-disabled-"))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out = frontmatter::render_flat(&keep);
    out.push('\n');
    out.push_str(body);
    out.into_bytes()
}

/// Flip / clear the skill disable sentinel. When `enabled = false`,
/// write the two disable keys (or refresh the timestamp). When
/// `enabled = true`, remove both. Preserves every other frontmatter
/// key verbatim.
pub fn set_skill_disabled(dir: &Path, enabled: bool, now: &str) -> AdapterResult<()> {
    let skill_file = dir.join(skill::SKILL_FILE);
    let bytes = rig_fs::read(&skill_file).map_err(|e| AdapterError::Other {
        message: format!("reading {}: {e}", skill_file.display()),
        source: Some(Box::new(e)),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| AdapterError::Other {
        message: format!("{} is not UTF-8: {e}", skill_file.display()),
        source: None,
    })?;

    // Split; fall back to a conservative error if the file has no FM.
    let (fm_block, body) = frontmatter::split(text).ok_or_else(|| AdapterError::Other {
        message: format!(
            "{} has no frontmatter fence; cannot toggle",
            skill_file.display()
        ),
        source: None,
    })?;

    let mut pairs: Vec<(String, String)> = frontmatter::parse_flat(fm_block);

    // Strip disable-related keys unconditionally.
    pairs.retain(|(k, _)| k != KEY_DISABLE_INVOCATION && !k.starts_with("rig-disabled-"));

    if !enabled {
        pairs.push((KEY_DISABLE_INVOCATION.to_owned(), "true".to_owned()));
        pairs.push((KEY_RIG_DISABLED_AT.to_owned(), now.to_owned()));
    }

    let borrowed: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut new_text = frontmatter::render_flat(&borrowed);
    new_text.push('\n');
    new_text.push_str(body);

    rig_fs::atomic_write(&skill_file, new_text.as_bytes()).map_err(|e| AdapterError::Other {
        message: format!("writing {}: {e}", skill_file.display()),
        source: Some(Box::new(e)),
    })
}

/// True if a skill directory's SKILL.md frontmatter carries the
/// `disable-model-invocation: true` flag.
pub fn skill_is_disabled(dir: &Path) -> AdapterResult<bool> {
    let skill_file = dir.join(skill::SKILL_FILE);
    if !skill_file.exists() {
        return Ok(false);
    }
    let bytes = rig_fs::read(&skill_file).map_err(|e| AdapterError::Other {
        message: format!("reading {}: {e}", skill_file.display()),
        source: Some(Box::new(e)),
    })?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(false);
    };
    let Some((fm, _)) = frontmatter::split(text) else {
        return Ok(false);
    };
    Ok(frontmatter::parse_flat(fm)
        .into_iter()
        .any(|(k, v)| k == KEY_DISABLE_INVOCATION && v == "true"))
}

// ---------------- single-file rename toggles ----------------

/// Toggle a file-backed unit (Rule / Command / Subagent) via rename.
pub fn set_file_disabled(
    scope: Scope,
    unit_type: UnitType,
    name: &str,
    enabled: bool,
) -> AdapterResult<()> {
    let active = primary_path(scope, unit_type, name)?;
    let disabled = add_suffix(&active);

    if enabled {
        // .rig-disabled → .md
        if active.exists() {
            // Collision with a non-Rig file.
            return Err(AdapterError::TargetCollision {
                path: active.display().to_string(),
            });
        }
        if !disabled.exists() {
            return Err(AdapterError::NotFound(name.to_owned(), scope));
        }
        std::fs::rename(&disabled, &active).map_err(|e| AdapterError::Other {
            message: format!(
                "renaming {} -> {}: {e}",
                disabled.display(),
                active.display()
            ),
            source: Some(Box::new(e)),
        })?;
    } else {
        // .md → .rig-disabled
        if disabled.exists() {
            // Already disabled — treat as idempotent no-op, but only
            // if the active path no longer exists (otherwise we'd be
            // overwriting user content).
            if active.exists() {
                return Err(AdapterError::TargetCollision {
                    path: disabled.display().to_string(),
                });
            }
            return Ok(());
        }
        if !active.exists() {
            return Err(AdapterError::NotFound(name.to_owned(), scope));
        }
        std::fs::rename(&active, &disabled).map_err(|e| AdapterError::Other {
            message: format!(
                "renaming {} -> {}: {e}",
                active.display(),
                disabled.display()
            ),
            source: Some(Box::new(e)),
        })?;
    }
    Ok(())
}

/// True if the disabled twin file exists on disk.
pub fn file_is_disabled(scope: Scope, unit_type: UnitType, name: &str) -> AdapterResult<bool> {
    let active = primary_path(scope, unit_type, name)?;
    Ok(add_suffix(&active).exists() && !active.exists())
}

/// Append `.rig-disabled` to an existing path.
#[must_use]
pub fn add_suffix(p: &Path) -> PathBuf {
    let mut out = p.as_os_str().to_owned();
    out.push(DISABLED_SUFFIX);
    PathBuf::from(out)
}

/// Strip `.rig-disabled` suffix if present.
#[must_use]
pub fn strip_suffix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(DISABLED_SUFFIX) {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

// ---------------- MCP snapshot ----------------

/// Disable an MCP by snapshotting the native entry to the Rig-owned
/// disabled dir then removing it via `claude mcp remove`.
pub fn disable_mcp(name: &str, scope: Scope) -> AdapterResult<()> {
    // Read the canonical form from the native config (best-effort).
    // If the entry is gone, nothing to do.
    let current = mcp::read_local(
        &rig_core::adapter::UnitRef::new(UnitType::Mcp, name.to_owned()),
        scope,
    );
    let m = match current {
        Ok(m) => m,
        Err(AdapterError::NotFound(_, _)) => {
            // Nothing to disable — either already disabled or never
            // installed. Treat as idempotent.
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Snapshot on disk.
    let snap_path = mcp_snapshot_path(scope, name)?;
    let snap = serde_json::json!({
        "schema": "rig/v1",
        "disabled_at": now_iso8601(),
        "agent": "claude",
        "scope": scope.to_string(),
        "config": mcp_to_snapshot_json(&m),
    });
    let bytes = serde_json::to_vec_pretty(&snap).map_err(|e| AdapterError::Other {
        message: format!("serialising mcp snapshot: {e}"),
        source: Some(Box::new(e)),
    })?;
    rig_fs::atomic_write(&snap_path, &bytes).map_err(|e| AdapterError::Other {
        message: format!("writing {}: {e}", snap_path.display()),
        source: Some(Box::new(e)),
    })?;

    // Remove from live config.
    mcp::uninstall(name, scope)
}

/// Enable an MCP by reading the snapshot and re-running
/// `claude mcp add` (via the shared install path).
pub fn enable_mcp(name: &str, scope: Scope) -> AdapterResult<()> {
    let snap_path = mcp_snapshot_path(scope, name)?;
    if !snap_path.exists() {
        return Err(AdapterError::NotFound(name.to_owned(), scope));
    }
    let bytes = rig_fs::read(&snap_path).map_err(|e| AdapterError::Other {
        message: format!("reading {}: {e}", snap_path.display()),
        source: Some(Box::new(e)),
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| AdapterError::Other {
        message: format!("parsing {}: {e}", snap_path.display()),
        source: Some(Box::new(e)),
    })?;
    let cfg = v.get("config").ok_or_else(|| AdapterError::Other {
        message: format!("{} missing `config`", snap_path.display()),
        source: None,
    })?;
    let mcp_unit = snapshot_json_to_mcp(name, cfg)?;
    mcp::install(&mcp_unit, scope).map(|_| ())?;
    // Delete the snapshot.
    let _ = std::fs::remove_file(&snap_path);
    Ok(())
}

/// Serialise an [`rig_core::unit::Mcp`] to the snapshot `config` JSON
/// shape — roughly the Claude-native entry shape.
pub fn mcp_to_snapshot_json(m: &rig_core::unit::Mcp) -> serde_json::Value {
    use rig_core::unit::Transport;
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(m.name.clone()));
    match &m.transport {
        Transport::Stdio { command, args } => {
            obj.insert("type".into(), serde_json::Value::String("stdio".into()));
            obj.insert("command".into(), serde_json::Value::String(command.clone()));
            obj.insert(
                "args".into(),
                serde_json::Value::Array(
                    args.iter()
                        .map(|a| serde_json::Value::String(a.clone()))
                        .collect(),
                ),
            );
        }
        Transport::Http { url, headers } => {
            obj.insert("type".into(), serde_json::Value::String("http".into()));
            obj.insert("url".into(), serde_json::Value::String(url.clone()));
            obj.insert(
                "headers".into(),
                serde_json::Value::Object(
                    headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect(),
                ),
            );
        }
        Transport::Sse { url, headers } => {
            obj.insert("type".into(), serde_json::Value::String("sse".into()));
            obj.insert("url".into(), serde_json::Value::String(url.clone()));
            obj.insert(
                "headers".into(),
                serde_json::Value::Object(
                    headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect(),
                ),
            );
        }
    }
    if !m.env.is_empty() {
        obj.insert(
            "env".into(),
            serde_json::Value::Array(
                m.env
                    .iter()
                    .map(|e| serde_json::Value::String(e.clone()))
                    .collect(),
            ),
        );
    }
    serde_json::Value::Object(obj)
}

/// Reverse of [`mcp_to_snapshot_json`]. Pure reconstructor so enable
/// doesn't need a live `claude mcp add-json` helper.
pub fn snapshot_json_to_mcp(
    name: &str,
    cfg: &serde_json::Value,
) -> AdapterResult<rig_core::unit::Mcp> {
    use rig_core::unit::Transport;
    use std::collections::BTreeMap;

    let obj = cfg.as_object().ok_or_else(|| AdapterError::Other {
        message: format!("snapshot `config` for `{name}` is not an object"),
        source: None,
    })?;
    let tag = obj
        .get("type")
        .or_else(|| obj.get("transport"))
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");
    let env: Vec<String> = obj
        .get("env")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let headers: BTreeMap<String, String> = obj
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let transport = match tag {
        "stdio" => {
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("snapshot for `{name}`: stdio missing `command`"),
                    source: None,
                })?
                .to_owned();
            let args = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            Transport::Stdio { command, args }
        }
        "http" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("snapshot for `{name}`: http missing `url`"),
                    source: None,
                })?
                .to_owned();
            Transport::Http { url, headers }
        }
        "sse" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("snapshot for `{name}`: sse missing `url`"),
                    source: None,
                })?
                .to_owned();
            Transport::Sse { url, headers }
        }
        other => {
            return Err(AdapterError::Other {
                message: format!("snapshot for `{name}`: unknown transport `{other}`"),
                source: None,
            });
        }
    };

    Ok(rig_core::unit::Mcp {
        name: name.to_owned(),
        description: None,
        transport,
        env,
        metadata: Default::default(),
    })
}

/// True if a snapshot exists for this MCP name under the scope.
pub fn mcp_is_disabled(scope: Scope, name: &str) -> AdapterResult<bool> {
    Ok(mcp_snapshot_path(scope, name)?.exists())
}

// Silence the unused-import lint when `scope_root` is compiled out.
#[allow(dead_code)]
fn _hold_scope_root(s: Scope) -> AdapterResult<PathBuf> {
    scope_root(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_format_is_stable() {
        assert_eq!(format_unix_secs(0), "1970-01-01T00:00:00Z");
        // 2020-01-01T00:00:00Z → 1577836800.
        assert_eq!(format_unix_secs(1_577_836_800), "2020-01-01T00:00:00Z");
    }

    #[test]
    fn normalise_strips_disable_keys() {
        let input = b"---\nname: x\ndescription: d\ndisable-model-invocation: true\nrig-disabled-at: 2026-04-21T10:14:22Z\n---\nbody\n";
        let out = normalise_skill_md(input);
        let text = std::str::from_utf8(&out).unwrap();
        assert!(!text.contains("disable-model-invocation"));
        assert!(!text.contains("rig-disabled-at"));
        assert!(text.contains("name: x"));
        assert!(text.contains("description: d"));
        assert!(text.ends_with("body\n"));
    }

    #[test]
    fn normalise_noop_when_no_disable_keys() {
        let input = b"---\nname: x\ndescription: d\n---\nbody\n";
        let out = normalise_skill_md(input);
        // Should produce byte-identical output after render round-trip.
        let rendered = std::str::from_utf8(&out).unwrap();
        assert!(rendered.contains("name: x"));
        assert!(rendered.contains("description: d"));
        assert!(rendered.ends_with("body\n"));
    }

    #[test]
    fn add_strip_suffix_roundtrip() {
        let p = Path::new("/a/b/x.md");
        assert_eq!(add_suffix(p), PathBuf::from("/a/b/x.md.rig-disabled"),);
        assert_eq!(strip_suffix(&add_suffix(p)), p);
        assert_eq!(strip_suffix(p), p); // already enabled
    }

    #[test]
    fn snapshot_json_roundtrip_stdio() {
        let m = rig_core::unit::Mcp {
            name: "gh".into(),
            description: None,
            transport: rig_core::unit::Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "srv".into()],
            },
            env: vec!["TOKEN".into()],
            metadata: Default::default(),
        };
        let j = mcp_to_snapshot_json(&m);
        let back = snapshot_json_to_mcp("gh", &j).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn snapshot_json_roundtrip_http() {
        let mut h = std::collections::BTreeMap::new();
        h.insert("X-K".into(), "v".into());
        let m = rig_core::unit::Mcp {
            name: "f".into(),
            description: None,
            transport: rig_core::unit::Transport::Http {
                url: "https://x/".into(),
                headers: h,
            },
            env: Vec::new(),
            metadata: Default::default(),
        };
        let j = mcp_to_snapshot_json(&m);
        let back = snapshot_json_to_mcp("f", &j).unwrap();
        assert_eq!(m, back);
    }
}
