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
mod frontmatter;
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
fn primary_path(scope: Scope, unit_type: UnitType, name: &str) -> AdapterResult<PathBuf> {
    let root = type_root(scope, unit_type)?;
    Ok(match unit_type {
        UnitType::Skill => root.join(name),
        _ => root.join(format!("{name}.md")),
    })
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
                    });
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
                        out.push(InstalledUnit {
                            unit_ref: UnitRef::new(ty, name),
                            scope,
                            paths: collect_files(&p).unwrap_or_else(|_| vec![skill_md]),
                        });
                    }
                    _ => {
                        if !ft.is_file() {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let Some(stem) = name.strip_suffix(".md") else {
                            continue;
                        };
                        out.push(InstalledUnit {
                            unit_ref: UnitRef::new(ty, stem.to_owned()),
                            scope,
                            paths: vec![p],
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
                if !primary.exists() {
                    return Err(AdapterError::NotFound(unit_ref.name.clone(), scope));
                }
                let bytes = rig_fs::read(&primary).map_err(to_other)?;
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
                        bytes.extend_from_slice(&rig_fs::read(&p).map_err(to_other)?);
                        bytes.push(0);
                    }
                    Some(Sha256::of(&bytes))
                }
            }
            _ => {
                if !primary.exists() {
                    None
                } else {
                    let file_name = primary.file_name().unwrap().to_string_lossy();
                    let mut bytes = Vec::new();
                    bytes.extend_from_slice(file_name.as_bytes());
                    bytes.push(0);
                    bytes.extend_from_slice(&rig_fs::read(&primary).map_err(to_other)?);
                    bytes.push(0);
                    Some(Sha256::of(&bytes))
                }
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
}
