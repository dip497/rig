//! Serde DTOs exposed over Tauri IPC. Flat, primitive-rich, camelCase
//! on the wire. Hand-mirrored on the TS side at `src/types.ts`.

use std::path::PathBuf;

use rig_core::drift::DriftState;
use rig_core::lockfile::Lockfile;
use rig_core::manifest::Manifest;
use rig_core::scope::Scope;
use rig_core::unit::UnitType;
use serde::{Deserialize, Serialize};

/// Scope selector as wire string. Maps to `rig_core::scope::Scope`.
pub type ScopeDto = Scope;

/// Unit type selector as wire string. Maps to `rig_core::unit::UnitType`.
pub type UnitTypeDto = UnitType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDto {
    pub id: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledUnitDto {
    pub agent: String,
    pub unit_type: String,
    pub name: String,
    pub paths: Vec<PathBuf>,
    /// Whether the unit is soft-disabled (`rig disable`). Additive,
    /// defaults to `false` so older clients stay backward-compatible.
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReportDto {
    pub state: DriftState,
    pub install_sha: Option<String>,
    pub current_sha: Option<String>,
    pub upstream_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitBodyDto {
    pub body: String,
    pub frontmatter: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDto {
    pub manifest: Manifest,
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileDto {
    pub lockfile: Lockfile,
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeRootsDto {
    pub global_rig: PathBuf,
    pub home: PathBuf,
    pub claude_global: PathBuf,
    pub codex_global: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResultDto {
    pub installed: Vec<InstalledUnitDto>,
    pub skipped: Vec<String>,
    pub source_sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MvResultDto {
    pub from_scope: Scope,
    pub to_scope: Scope,
    pub install_sha: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResultDto {
    pub installed: Vec<InstalledUnitDto>,
    pub skipped: Vec<String>,
    pub conflicts: Vec<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStatsDto {
    pub unit_type: String,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatsDto {
    pub agent: String,
    pub by_type: Vec<TypeStatsDto>,
    pub total_count: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDto {
    pub agents: Vec<AgentStatsDto>,
    pub grand_total_count: u64,
    pub grand_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateLocationDto {
    pub agent: String,
    pub scope: Scope,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateDto {
    pub unit_type: String,
    pub name: String,
    pub locations: Vec<DuplicateLocationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorResultDto {
    pub duplicates: Vec<DuplicateDto>,
    pub broken_symlinks: Vec<String>,
    pub mv_split: Vec<String>,
    pub mv_stale_lock: Vec<String>,
    pub fixed: u32,
}

pub fn unit_type_slug(t: UnitType) -> &'static str {
    match t {
        UnitType::Skill => "skill",
        UnitType::Mcp => "mcp",
        UnitType::Rule => "rule",
        UnitType::Hook => "hook",
        UnitType::Command => "command",
        UnitType::Subagent => "subagent",
        UnitType::Plugin => "plugin",
    }
}
