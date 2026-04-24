//! `rig-adapter-claude` — Claude Code adapter.
//!
//! Translates canonical units into `~/.claude/` (global) or
//! `./.claude/` (project) layouts.
//!
//! Supported unit types:
//! - `Skill`    → `<scope>/skills/<name>/SKILL.md` (+ resources)
//! - `Rule`     → `<scope>/rules/<name>.md`
//! - `Command`  → `<scope>/commands/<name>.md`
//! - `Subagent` → `<scope>/agents/<name>.md`
//!
//! MCP, Hook, and Plugin land in later wedges (they mutate
//! `settings.json` or delegate to `claude plugin install`).

#![forbid(unsafe_code)]

mod command;
pub mod disabled;
pub mod frontmatter;
pub mod mcp;
mod rule;
mod skill;
mod subagent;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rig_core::adapter::{Adapter, AdapterError, AdapterResult, InstalledUnit, Receipt, UnitRef};
use rig_core::agent::AgentId;
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::drift::{DriftShas, DriftState};
use rig_core::scope::Scope;
use rig_core::source::Sha256;
use rig_core::unit::{Unit, UnitType};

pub use command::CommandConverter;
pub use mcp::MCPConverter;
pub use rule::RuleConverter;
pub use skill::SkillConverter;
pub use subagent::SubagentConverter;

pub const AGENT_ID: &str = "claude";

/// Subdirectory under `<scope>/.claude/` where a given unit type lives.
fn subdir(unit_type: UnitType) -> AdapterResult<&'static str> {
    Ok(match unit_type {
        UnitType::Skill => "skills",
        UnitType::Rule => "rules",
        UnitType::Command => "commands",
        UnitType::Subagent => "agents",
        other => return Err(AdapterError::Unsupported(other)),
    })
}

fn scope_root(scope: Scope) -> AdapterResult<PathBuf> {
    match scope {
        Scope::Global => {
            let home = rig_fs::home_dir().map_err(to_other)?;
            Ok(home.join(".claude"))
        }
        Scope::Project => Ok(PathBuf::from(".claude")),
        // `Local` is MCP-only; MCP ops bypass `scope_root`. If a file-
        // backed path is requested under `Local`, it's a bug — fail
        // loud per `docs/MCP-SUPPORT.md` §8.
        Scope::Local => Err(AdapterError::Other {
            message: "scope `local` is only supported for MCP units on claude".into(),
            source: None,
        }),
    }
}

fn type_root(scope: Scope, unit_type: UnitType) -> AdapterResult<PathBuf> {
    Ok(scope_root(scope)?.join(subdir(unit_type)?))
}

/// Where the primary file of a unit lives on disk. For skills it's a
/// directory; for everything else it's a single `.md` file.
pub(crate) fn primary_path(
    scope: Scope,
    unit_type: UnitType,
    name: &str,
) -> AdapterResult<PathBuf> {
    let root = type_root(scope, unit_type)?;
    Ok(match unit_type {
        UnitType::Skill => root.join(name),
        _ => root.join(format!("{name}.md")),
    })
}

/// Directory where Rig stores MCP disable snapshots for this scope.
/// `<scope>/.rig/disabled/mcp/`.
fn disabled_mcp_dir(scope: Scope) -> AdapterResult<PathBuf> {
    let base = match scope {
        Scope::Global => rig_fs::home_dir()
            .map(|h| h.join(".rig"))
            .map_err(to_other)?,
        Scope::Project | Scope::Local => PathBuf::from(".rig"),
    };
    Ok(base.join("disabled").join("mcp"))
}

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for ClaudeAdapter {
    fn agent(&self) -> AgentId {
        AgentId::new(AGENT_ID)
    }

    fn capabilities(&self) -> BTreeSet<UnitType> {
        [
            UnitType::Skill,
            UnitType::Rule,
            UnitType::Command,
            UnitType::Subagent,
            UnitType::Mcp,
        ]
        .into_iter()
        .collect()
    }

    fn install(&self, unit: &Unit, scope: Scope) -> AdapterResult<Receipt> {
        // MCP units route through `claude mcp add` instead of writing
        // files directly. Spec §4.
        if let Unit::Mcp(m) = unit {
            mcp::validate_scope(UnitType::Mcp, scope)?;
            let (install_sha, path) = mcp::install(m, scope)?;
            return Ok(Receipt {
                unit_ref: UnitRef::new(UnitType::Mcp, m.name.clone()),
                agent: self.agent(),
                scope,
                paths: vec![path],
                install_sha,
            });
        }

        // Any non-MCP unit under `Scope::Local` is invalid.
        mcp::validate_scope(unit.unit_type(), scope)?;

        let (unit_type, name, native) = to_native(unit)?;

        let (install_root, paths, install_sha) = match unit_type {
            UnitType::Skill => {
                // Skills are directories: write all files under
                // `<type_root>/<name>/`.
                let dir = type_root(scope, unit_type)?.join(&name);
                let mut paths = Vec::with_capacity(native.files.len());
                let mut hash_input = Vec::new();
                for f in &native.files {
                    let p = dir.join(&f.relative_path);
                    rig_fs::atomic_write(&p, &f.bytes).map_err(to_other)?;
                    hash_input.extend_from_slice(f.relative_path.as_bytes());
                    hash_input.push(0);
                    hash_input.extend_from_slice(&f.bytes);
                    hash_input.push(0);
                    paths.push(p);
                }
                (dir, paths, Sha256::of(&hash_input))
            }
            _ => {
                // Single-file types: converter emits exactly one file.
                let f = native.files.first().ok_or_else(|| AdapterError::Other {
                    message: format!("{unit_type:?}: converter produced no files"),
                    source: None,
                })?;
                let p = type_root(scope, unit_type)?.join(&f.relative_path);
                rig_fs::atomic_write(&p, &f.bytes).map_err(to_other)?;
                let mut hash_input = Vec::new();
                hash_input.extend_from_slice(f.relative_path.as_bytes());
                hash_input.push(0);
                hash_input.extend_from_slice(&f.bytes);
                hash_input.push(0);
                (p.clone(), vec![p], Sha256::of(&hash_input))
            }
        };
        let _ = install_root;

        Ok(Receipt {
            unit_ref: UnitRef::new(unit_type, name),
            agent: self.agent(),
            scope,
            paths,
            install_sha,
        })
    }

    fn uninstall(&self, unit_ref: &UnitRef, scope: Scope) -> AdapterResult<()> {
        if unit_ref.unit_type == UnitType::Mcp {
            mcp::validate_scope(UnitType::Mcp, scope)?;
            // Clean up any disable snapshot too.
            if let Ok(snap) = disabled::mcp_snapshot_path(scope, &unit_ref.name) {
                let _ = std::fs::remove_file(&snap);
            }
            return mcp::uninstall(&unit_ref.name, scope);
        }
        mcp::validate_scope(unit_ref.unit_type, scope)?;
        let p = primary_path(scope, unit_ref.unit_type, &unit_ref.name)?;
        match unit_ref.unit_type {
            UnitType::Skill => {
                if p.exists() {
                    std::fs::remove_dir_all(&p).map_err(|e| AdapterError::Other {
                        message: format!("removing {}", p.display()),
                        source: Some(Box::new(e)),
                    })?;
                }
            }
            _ => {
                rig_fs::remove_if_exists(&p).map_err(to_other)?;
                // Also remove any disabled twin.
                let d = disabled::add_suffix(&p);
                rig_fs::remove_if_exists(&d).map_err(to_other)?;
            }
        }
        Ok(())
    }

    fn list(&self, scope: Scope) -> AdapterResult<Vec<InstalledUnit>> {
        let mut out = Vec::new();

        // MCP entries: enumerate native config. Higher-level callers
        // (CLI `list`) intersect with the lockfile to hide foreign
        // entries per `docs/MCP-SUPPORT.md` §6.
        if matches!(scope, Scope::Global | Scope::Project | Scope::Local) {
            if let Ok(mcps) = mcp::list_native(scope) {
                let cfg = mcp::config_path(scope)?;
                for m in mcps {
                    out.push(InstalledUnit {
                        unit_ref: UnitRef::new(UnitType::Mcp, m.name),
                        scope,
                        paths: vec![cfg.clone()],
                        disabled: false,
                    });
                }
            }
            // Disabled MCP entries live under `<scope>/.rig/disabled/mcp/`
            // as snapshot files; surface them too.
            if let Ok(dir) = disabled_mcp_dir(scope) {
                if dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for e in entries.flatten() {
                            let name_os = e.file_name();
                            let file_name = name_os.to_string_lossy();
                            // We own <name>.claude.json.
                            if let Some(stem) = file_name.strip_suffix(".claude.json") {
                                out.push(InstalledUnit {
                                    unit_ref: UnitRef::new(UnitType::Mcp, stem.to_owned()),
                                    scope,
                                    paths: vec![e.path()],
                                    disabled: true,
                                });
                            }
                        }
                    }
                }
            }
        }

        // File-backed unit types don't exist under `Scope::Local`.
        if matches!(scope, Scope::Local) {
            return Ok(out);
        }

        for ty in [
            UnitType::Skill,
            UnitType::Rule,
            UnitType::Command,
            UnitType::Subagent,
        ] {
            let root = type_root(scope, ty)?;
            if !root.exists() {
                continue;
            }
            let entries = std::fs::read_dir(&root).map_err(|e| AdapterError::Other {
                message: format!("reading {}", root.display()),
                source: Some(Box::new(e)),
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| AdapterError::Other {
                    message: format!("reading {}", root.display()),
                    source: Some(Box::new(e)),
                })?;
                let p = entry.path();
                let ft = entry.file_type().map_err(to_io)?;

                match ty {
                    UnitType::Skill => {
                        if !ft.is_dir() {
                            continue;
                        }
                        let skill_md = p.join(skill::SKILL_FILE);
                        if !skill_md.exists() {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let disabled = disabled::skill_is_disabled(&p).unwrap_or(false);
                        out.push(InstalledUnit {
                            unit_ref: UnitRef::new(ty, name),
                            scope,
                            paths: collect_files(&p).unwrap_or_else(|_| vec![skill_md]),
                            disabled,
                        });
                    }
                    _ => {
                        if !ft.is_file() {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        // Handle both `<stem>.md` and
                        // `<stem>.md.rig-disabled` entries.
                        let (stem, is_disabled) =
                            if let Some(stem) = name.strip_suffix(".md.rig-disabled") {
                                (stem.to_owned(), true)
                            } else if let Some(stem) = name.strip_suffix(".md") {
                                (stem.to_owned(), false)
                            } else {
                                continue;
                            };
                        out.push(InstalledUnit {
                            unit_ref: UnitRef::new(ty, stem),
                            scope,
                            paths: vec![p],
                            disabled: is_disabled,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    fn read_local(&self, unit_ref: &UnitRef, scope: Scope) -> AdapterResult<Unit> {
        if unit_ref.unit_type == UnitType::Mcp {
            mcp::validate_scope(UnitType::Mcp, scope)?;
            return Ok(Unit::Mcp(mcp::read_local(unit_ref, scope)?));
        }
        mcp::validate_scope(unit_ref.unit_type, scope)?;
        let primary = primary_path(scope, unit_ref.unit_type, &unit_ref.name)?;
        let native = match unit_ref.unit_type {
            UnitType::Skill => {
                if !primary.join(skill::SKILL_FILE).exists() {
                    return Err(AdapterError::NotFound(unit_ref.name.clone(), scope));
                }
                let files = collect_files(&primary).map_err(to_other)?;
                let mut out = Vec::with_capacity(files.len());
                for p in files {
                    let rel = p
                        .strip_prefix(&primary)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .into_owned();
                    let bytes = rig_fs::read(&p).map_err(to_other)?;
                    out.push(NativeFile {
                        relative_path: rel,
                        bytes,
                    });
                }
                NativeLayout { files: out }
            }
            _ => {
                // Follow the `.rig-disabled` rename suffix so
                // `read_local` works for disabled units.
                let disabled_p = disabled::add_suffix(&primary);
                let read_path = if primary.exists() {
                    primary.clone()
                } else if disabled_p.exists() {
                    disabled_p
                } else {
                    return Err(AdapterError::NotFound(unit_ref.name.clone(), scope));
                };
                let bytes = rig_fs::read(&read_path).map_err(to_other)?;
                NativeLayout {
                    files: vec![NativeFile {
                        relative_path: primary.file_name().unwrap().to_string_lossy().into_owned(),
                        bytes,
                    }],
                }
            }
        };

        from_native(unit_ref.unit_type, &native)
    }

    fn set_enabled(&self, unit_ref: &UnitRef, scope: Scope, enabled: bool) -> AdapterResult<()> {
        match unit_ref.unit_type {
            UnitType::Skill => {
                mcp::validate_scope(unit_ref.unit_type, scope)?;
                let dir = primary_path(scope, UnitType::Skill, &unit_ref.name)?;
                if !dir.join(skill::SKILL_FILE).exists() {
                    return Err(AdapterError::NotFound(unit_ref.name.clone(), scope));
                }
                disabled::set_skill_disabled(&dir, enabled, &disabled::now_iso8601())
            }
            UnitType::Rule | UnitType::Command | UnitType::Subagent => {
                mcp::validate_scope(unit_ref.unit_type, scope)?;
                disabled::set_file_disabled(scope, unit_ref.unit_type, &unit_ref.name, enabled)
            }
            UnitType::Mcp => {
                mcp::validate_scope(UnitType::Mcp, scope)?;
                if enabled {
                    disabled::enable_mcp(&unit_ref.name, scope)
                } else {
                    disabled::disable_mcp(&unit_ref.name, scope)
                }
            }
            UnitType::Hook | UnitType::Plugin => Err(AdapterError::UnsupportedOp("enable/disable")),
        }
    }

    fn is_enabled(&self, unit_ref: &UnitRef, scope: Scope) -> AdapterResult<bool> {
        match unit_ref.unit_type {
            UnitType::Skill => {
                mcp::validate_scope(unit_ref.unit_type, scope)?;
                let dir = primary_path(scope, UnitType::Skill, &unit_ref.name)?;
                Ok(!disabled::skill_is_disabled(&dir)?)
            }
            UnitType::Rule | UnitType::Command | UnitType::Subagent => {
                mcp::validate_scope(unit_ref.unit_type, scope)?;
                Ok(!disabled::file_is_disabled(
                    scope,
                    unit_ref.unit_type,
                    &unit_ref.name,
                )?)
            }
            UnitType::Mcp => {
                mcp::validate_scope(UnitType::Mcp, scope)?;
                Ok(!disabled::mcp_is_disabled(scope, &unit_ref.name)?)
            }
            UnitType::Hook | UnitType::Plugin => Err(AdapterError::UnsupportedOp("enable/disable")),
        }
    }

    fn detect_drift(
        &self,
        unit_ref: &UnitRef,
        scope: Scope,
        install_time: Sha256,
        upstream: Option<Sha256>,
    ) -> AdapterResult<(DriftState, DriftShas)> {
        if unit_ref.unit_type == UnitType::Mcp {
            mcp::validate_scope(UnitType::Mcp, scope)?;
            let current = mcp::current_sha(&unit_ref.name, scope)?;
            let shas = DriftShas {
                install_time,
                current_disk: current.clone(),
                upstream,
            };
            let state = if current.is_none() {
                DriftState::Missing
            } else {
                shas.classify()
            };
            return Ok((state, shas));
        }
        mcp::validate_scope(unit_ref.unit_type, scope)?;
        let primary = primary_path(scope, unit_ref.unit_type, &unit_ref.name)?;
        let current = match unit_ref.unit_type {
            UnitType::Skill => {
                if !primary.join(skill::SKILL_FILE).exists() {
                    None
                } else {
                    let files = collect_files(&primary).map_err(to_other)?;
                    let mut bytes = Vec::new();
                    for p in files {
                        let rel = p.strip_prefix(&primary).unwrap_or(&p).to_string_lossy();
                        bytes.extend_from_slice(rel.as_bytes());
                        bytes.push(0);
                        let raw = rig_fs::read(&p).map_err(to_other)?;
                        // Normalise SKILL.md to strip Rig's own
                        // disable flip so a disabled skill stays Clean.
                        let normalised =
                            if p.file_name().and_then(|s| s.to_str()) == Some(skill::SKILL_FILE) {
                                disabled::normalise_skill_md(&raw)
                            } else {
                                raw
                            };
                        bytes.extend_from_slice(&normalised);
                        bytes.push(0);
                    }
                    Some(Sha256::of(&bytes))
                }
            }
            _ => {
                // Follow the `.rig-disabled` rename suffix if present —
                // the unit's canonical filename (pre-disable) is what
                // the install_sha was computed against.
                let disabled_path = disabled::add_suffix(&primary);
                let (read_path, active_name) = if primary.exists() {
                    (primary.clone(), primary.file_name().unwrap().to_os_string())
                } else if disabled_path.exists() {
                    // Strip the `.rig-disabled` suffix from the hashed
                    // filename so the SHA matches install-time bytes.
                    (disabled_path, primary.file_name().unwrap().to_os_string())
                } else {
                    return Ok((
                        DriftState::Missing,
                        DriftShas {
                            install_time,
                            current_disk: None,
                            upstream,
                        },
                    ));
                };
                let file_name = active_name.to_string_lossy();
                let mut bytes = Vec::new();
                bytes.extend_from_slice(file_name.as_bytes());
                bytes.push(0);
                bytes.extend_from_slice(&rig_fs::read(&read_path).map_err(to_other)?);
                bytes.push(0);
                Some(Sha256::of(&bytes))
            }
        };

        let shas = DriftShas {
            install_time,
            current_disk: current.clone(),
            upstream,
        };
        let state = if current.is_none() {
            DriftState::Missing
        } else {
            shas.classify()
        };
        Ok((state, shas))
    }
}

fn to_native(unit: &Unit) -> AdapterResult<(UnitType, String, NativeLayout)> {
    match unit {
        Unit::Skill(u) => Ok((
            UnitType::Skill,
            u.name.clone(),
            SkillConverter.to_native(u)?,
        )),
        Unit::Rule(u) => Ok((UnitType::Rule, u.name.clone(), RuleConverter.to_native(u)?)),
        Unit::Command(u) => Ok((
            UnitType::Command,
            u.name.clone(),
            CommandConverter.to_native(u)?,
        )),
        Unit::Subagent(u) => Ok((
            UnitType::Subagent,
            u.name.clone(),
            SubagentConverter.to_native(u)?,
        )),
        Unit::Mcp(u) => Ok((UnitType::Mcp, u.name.clone(), MCPConverter.to_native(u)?)),
        _ => Err(AdapterError::Unsupported(unit.unit_type())),
    }
}

fn from_native(unit_type: UnitType, native: &NativeLayout) -> AdapterResult<Unit> {
    Ok(match unit_type {
        UnitType::Skill => Unit::Skill(SkillConverter.parse_native(native)?),
        UnitType::Rule => Unit::Rule(RuleConverter.parse_native(native)?),
        UnitType::Command => Unit::Command(CommandConverter.parse_native(native)?),
        UnitType::Subagent => Unit::Subagent(SubagentConverter.parse_native(native)?),
        UnitType::Mcp => Unit::Mcp(MCPConverter.parse_native(native)?),
        other => return Err(AdapterError::Unsupported(other)),
    })
}

fn collect_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk(&p, out)?;
        } else if ft.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

fn to_other<E: std::error::Error + Send + Sync + 'static>(e: E) -> AdapterError {
    AdapterError::Other {
        message: e.to_string(),
        source: Some(Box::new(e)),
    }
}

fn to_io(e: std::io::Error) -> AdapterError {
    AdapterError::Other {
        message: e.to_string(),
        source: Some(Box::new(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::unit::{Rule, Skill, Subagent};

    use std::sync::Mutex;
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    fn tempdir(tag: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let p = std::env::temp_dir().join(format!(
            "rig-adapter-claude-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn with_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("HOME");
        std::env::set_var("HOME", home);
        let r = f();
        match prev {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        r
    }

    fn sample_skill() -> Skill {
        Skill {
            name: "sample".into(),
            description: "does x".into(),
            extra_frontmatter: Default::default(),
            body: "# sample\n".into(),
            resources: Vec::new(),
        }
    }

    #[test]
    fn skill_roundtrip() {
        let tmp = tempdir("skill");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let unit = Unit::Skill(sample_skill());

            let r = adapter.install(&unit, Scope::Global).unwrap();
            assert!(r.paths[0].ends_with("sample/SKILL.md"));
            assert_eq!(
                adapter.read_local(&r.unit_ref, Scope::Global).unwrap(),
                unit
            );
            adapter.uninstall(&r.unit_ref, Scope::Global).unwrap();
            assert!(adapter.list(Scope::Global).unwrap().is_empty());
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn rule_roundtrip() {
        let tmp = tempdir("rule");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let unit = Unit::Rule(Rule {
                name: "ts-style".into(),
                description: Some("TS rules".into()),
                body: "use const\n".into(),
                placement: Default::default(),
            });

            let r = adapter.install(&unit, Scope::Global).unwrap();
            assert!(r.paths[0].ends_with("rules/ts-style.md"));
            let back = adapter.read_local(&r.unit_ref, Scope::Global).unwrap();
            assert_eq!(back, unit);

            let listed = adapter.list(Scope::Global).unwrap();
            assert!(listed.iter().any(|u| u.unit_ref.name == "ts-style"));

            adapter.uninstall(&r.unit_ref, Scope::Global).unwrap();
            assert!(!r.paths[0].exists());
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn subagent_roundtrip() {
        let tmp = tempdir("subagent");
        with_home(&tmp, || {
            let adapter = ClaudeAdapter::new();
            let unit = Unit::Subagent(Subagent {
                name: "sec".into(),
                description: "sec review".into(),
                tools: vec!["Read".into(), "Grep".into()],
                model: Some("opus".into()),
                body: "do the thing\n".into(),
            });

            let r = adapter.install(&unit, Scope::Global).unwrap();
            assert!(r.paths[0].ends_with("agents/sec.md"));
            assert_eq!(
                adapter.read_local(&r.unit_ref, Scope::Global).unwrap(),
                unit
            );
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn list_spans_all_types() {
        let tmp = tempdir("list-all");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            a.install(&Unit::Skill(sample_skill()), Scope::Global)
                .unwrap();
            a.install(
                &Unit::Rule(Rule {
                    name: "r".into(),
                    description: None,
                    body: "x\n".into(),
                    placement: Default::default(),
                }),
                Scope::Global,
            )
            .unwrap();
            let listed = a.list(Scope::Global).unwrap();
            assert_eq!(listed.len(), 2);
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn unsupported_unit_errors() {
        // Hook and Plugin remain Unsupported in M1.
        let adapter = ClaudeAdapter::new();
        let hook = Unit::Hook(rig_core::unit::Hook {
            name: "x".into(),
            event: rig_core::unit::HookEvent::PreToolUse,
            matcher: None,
            command: "echo".into(),
            description: None,
        });
        assert!(matches!(
            adapter.install(&hook, Scope::Project),
            Err(AdapterError::Unsupported(UnitType::Hook))
        ));
    }

    #[test]
    fn mcp_is_supported_in_capabilities() {
        let adapter = ClaudeAdapter::new();
        assert!(adapter.capabilities().contains(&UnitType::Mcp));
    }

    #[test]
    fn local_scope_rejected_for_non_mcp() {
        let adapter = ClaudeAdapter::new();
        let rule = Unit::Rule(rig_core::unit::Rule {
            name: "r".into(),
            description: None,
            body: "x\n".into(),
            placement: Default::default(),
        });
        assert!(matches!(
            adapter.install(&rule, Scope::Local),
            Err(AdapterError::Unsupported(UnitType::Rule))
        ));
    }

    // ---------------- Wedge B: enable / disable ----------------

    #[test]
    fn disable_enable_rule_stays_clean() {
        let tmp = tempdir("disable-rule");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            let unit = Unit::Rule(rig_core::unit::Rule {
                name: "ts-strict".into(),
                description: None,
                body: "rule body\n".into(),
                placement: Default::default(),
            });
            let r = a.install(&unit, Scope::Global).unwrap();
            let install_sha = r.install_sha.clone();

            // Baseline: Clean.
            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha.clone(), None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);

            // Disable.
            a.set_enabled(&r.unit_ref, Scope::Global, false).unwrap();
            assert!(!a.is_enabled(&r.unit_ref, Scope::Global).unwrap());
            // Drift stays Clean after disable.
            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha.clone(), None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);

            // list() surfaces with disabled=true.
            let listed = a.list(Scope::Global).unwrap();
            let entry = listed
                .iter()
                .find(|u| u.unit_ref.name == "ts-strict")
                .unwrap();
            assert!(entry.disabled);

            // Enable.
            a.set_enabled(&r.unit_ref, Scope::Global, true).unwrap();
            assert!(a.is_enabled(&r.unit_ref, Scope::Global).unwrap());
            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha, None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn disable_enable_skill_stays_clean() {
        let tmp = tempdir("disable-skill");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            let unit = Unit::Skill(sample_skill());
            let r = a.install(&unit, Scope::Global).unwrap();
            let install_sha = r.install_sha.clone();

            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha.clone(), None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);

            a.set_enabled(&r.unit_ref, Scope::Global, false).unwrap();
            assert!(!a.is_enabled(&r.unit_ref, Scope::Global).unwrap());
            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha.clone(), None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);

            a.set_enabled(&r.unit_ref, Scope::Global, true).unwrap();
            assert!(a.is_enabled(&r.unit_ref, Scope::Global).unwrap());
            let (st, _) = a
                .detect_drift(&r.unit_ref, Scope::Global, install_sha, None)
                .unwrap();
            assert_eq!(st, DriftState::Clean);
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn enable_collision_errors() {
        let tmp = tempdir("collision");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            let unit = Unit::Rule(rig_core::unit::Rule {
                name: "x".into(),
                description: None,
                body: "x\n".into(),
                placement: Default::default(),
            });
            let r = a.install(&unit, Scope::Global).unwrap();
            a.set_enabled(&r.unit_ref, Scope::Global, false).unwrap();

            // User creates a non-Rig file at the active path.
            let active = &r.paths[0];
            std::fs::write(active, b"user-content\n").unwrap();

            let err = a.set_enabled(&r.unit_ref, Scope::Global, true).unwrap_err();
            assert!(matches!(err, AdapterError::TargetCollision { .. }));
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn command_and_subagent_toggle_roundtrip() {
        let tmp = tempdir("toggle-cmd-sub");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            let cmd = Unit::Command(rig_core::unit::Command {
                name: "review".into(),
                description: None,
                body: "run review\n".into(),
                tools: vec![],
            });
            let sub = Unit::Subagent(rig_core::unit::Subagent {
                name: "sec".into(),
                description: "d".into(),
                tools: vec![],
                model: None,
                body: "b\n".into(),
            });
            let rc = a.install(&cmd, Scope::Global).unwrap();
            let rs = a.install(&sub, Scope::Global).unwrap();

            a.set_enabled(&rc.unit_ref, Scope::Global, false).unwrap();
            a.set_enabled(&rs.unit_ref, Scope::Global, false).unwrap();
            let (st1, _) = a
                .detect_drift(&rc.unit_ref, Scope::Global, rc.install_sha.clone(), None)
                .unwrap();
            let (st2, _) = a
                .detect_drift(&rs.unit_ref, Scope::Global, rs.install_sha.clone(), None)
                .unwrap();
            assert_eq!(st1, DriftState::Clean);
            assert_eq!(st2, DriftState::Clean);
            a.set_enabled(&rc.unit_ref, Scope::Global, true).unwrap();
            a.set_enabled(&rs.unit_ref, Scope::Global, true).unwrap();
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn hook_plugin_toggle_unsupported() {
        let a = ClaudeAdapter::new();
        let r = a.set_enabled(&UnitRef::new(UnitType::Hook, "x"), Scope::Global, false);
        assert!(matches!(r, Err(AdapterError::UnsupportedOp(_))));
    }

    #[test]
    fn skill_extra_frontmatter_survives_disable_enable() {
        let tmp = tempdir("extra-fm");
        with_home(&tmp, || {
            let a = ClaudeAdapter::new();
            let mut extra = std::collections::BTreeMap::new();
            extra.insert("author".to_owned(), toml::Value::String("acme".into()));
            extra.insert("license".to_owned(), toml::Value::String("MIT".into()));
            let sk = rig_core::unit::Skill {
                name: "rr".into(),
                description: "d".into(),
                extra_frontmatter: extra.clone(),
                body: "body\n".into(),
                resources: Vec::new(),
            };
            let r = a.install(&Unit::Skill(sk.clone()), Scope::Global).unwrap();
            a.set_enabled(&r.unit_ref, Scope::Global, false).unwrap();
            a.set_enabled(&r.unit_ref, Scope::Global, true).unwrap();
            let back = a.read_local(&r.unit_ref, Scope::Global).unwrap();
            if let Unit::Skill(s) = back {
                assert_eq!(s.extra_frontmatter, extra);
            } else {
                panic!("not a skill");
            }
        });
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn mcp_json_reconstructor_roundtrip_per_transport() {
        // Pure function — no filesystem. Covers all three transports.
        let stdio = rig_core::unit::Mcp {
            name: "gh".into(),
            description: None,
            transport: rig_core::unit::Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "s".into()],
            },
            env: vec!["T".into()],
            metadata: Default::default(),
        };
        let http = rig_core::unit::Mcp {
            name: "f".into(),
            description: None,
            transport: rig_core::unit::Transport::Http {
                url: "https://x/".into(),
                headers: [("K".to_owned(), "v".to_owned())].into_iter().collect(),
            },
            env: Vec::new(),
            metadata: Default::default(),
        };
        let sse = rig_core::unit::Mcp {
            name: "s".into(),
            description: None,
            transport: rig_core::unit::Transport::Sse {
                url: "https://y/".into(),
                headers: Default::default(),
            },
            env: Vec::new(),
            metadata: Default::default(),
        };
        for m in [stdio, http, sse] {
            let j = disabled::mcp_to_snapshot_json(&m);
            let back = disabled::snapshot_json_to_mcp(&m.name, &j).unwrap();
            assert_eq!(back, m);
        }
    }
}
