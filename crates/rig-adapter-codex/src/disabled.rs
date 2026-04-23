//! Enable / disable mechanics for the Codex adapter.
//!
//! Codex has no native frontmatter disable flag for skills (unlike
//! Claude), so skills use the same rename trick as rules / commands /
//! subagents. MCP entries snapshot to
//! `<scope>/.rig/disabled/mcp/<name>.codex.json` and get removed from
//! `~/.codex/config.toml` via `codex mcp remove`.

use std::path::{Path, PathBuf};

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::scope::Scope;
use rig_core::unit::UnitType;

use crate::{mcp, primary_path, skill};

pub const DISABLED_SUFFIX: &str = ".rig-disabled";

#[must_use]
pub fn add_suffix(p: &Path) -> PathBuf {
    let mut out = p.as_os_str().to_owned();
    out.push(DISABLED_SUFFIX);
    PathBuf::from(out)
}

#[must_use]
pub fn strip_suffix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(DISABLED_SUFFIX) {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// Compute `<scope>/.rig/disabled/mcp/<name>.codex.json`.
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
        .join(format!("{name}.codex.json")))
}

// ---------------- file-backed toggle ----------------

/// Toggle a file/dir-backed Codex unit (Skill / Rule / Command /
/// Subagent) via rename. For skills, the `SKILL.md` inside the skill
/// directory is renamed (since Codex enumerates skills by the
/// `SKILL.md` marker).
pub fn set_file_disabled(
    scope: Scope,
    unit_type: UnitType,
    name: &str,
    enabled: bool,
) -> AdapterResult<()> {
    let target_path = match unit_type {
        UnitType::Skill => primary_path(scope, UnitType::Skill, name)?.join(skill::SKILL_FILE),
        UnitType::Rule | UnitType::Command | UnitType::Subagent => {
            primary_path(scope, unit_type, name)?
        }
        other => {
            return Err(AdapterError::UnsupportedOp(match other {
                UnitType::Hook => "enable/disable (hook)",
                UnitType::Plugin => "enable/disable (plugin)",
                _ => "enable/disable",
            }))
        }
    };

    let active = target_path.clone();
    let disabled = add_suffix(&active);

    if enabled {
        if active.exists() {
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
        if disabled.exists() {
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

/// Is the file-backed unit currently disabled (disabled twin exists,
/// active does not)?
pub fn file_is_disabled(scope: Scope, unit_type: UnitType, name: &str) -> AdapterResult<bool> {
    let active = match unit_type {
        UnitType::Skill => primary_path(scope, UnitType::Skill, name)?.join(skill::SKILL_FILE),
        _ => primary_path(scope, unit_type, name)?,
    };
    Ok(add_suffix(&active).exists() && !active.exists())
}

// ---------------- MCP snapshot ----------------

pub fn disable_mcp(name: &str, scope: Scope) -> AdapterResult<()> {
    // Codex MCP lives only under Scope::Global, per the adapter.
    let current = mcp::read_local(
        &rig_core::adapter::UnitRef::new(UnitType::Mcp, name.to_owned()),
        scope,
    );
    let m = match current {
        Ok(m) => m,
        Err(AdapterError::NotFound(_, _)) => return Ok(()),
        Err(e) => return Err(e),
    };
    let snap_path = mcp_snapshot_path(scope, name)?;
    let snap = serde_json::json!({
        "schema": "rig/v1",
        "agent": "codex",
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
    mcp::uninstall(name, scope)
}

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
    let _ = std::fs::remove_file(&snap_path);
    Ok(())
}

pub fn mcp_is_disabled(scope: Scope, name: &str) -> AdapterResult<bool> {
    Ok(mcp_snapshot_path(scope, name)?.exists())
}

/// Serialise an [`rig_core::unit::Mcp`] to the snapshot `config` JSON.
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
        Transport::Http { url, headers } | Transport::Sse { url, headers } => {
            obj.insert(
                "type".into(),
                serde_json::Value::String(if matches!(&m.transport, Transport::Http { .. }) {
                    "http".into()
                } else {
                    "sse".into()
                }),
            );
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
