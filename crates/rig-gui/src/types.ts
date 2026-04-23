// Hand-mirrored DTOs matching crates/rig-gui/src-tauri/src/dto.rs.
// Keep in sync when Rust side changes.

export type Scope = "global" | "project" | "local";
export type ScopeSelection = Scope | "all";

export type UnitTypeId =
  | "skill"
  | "mcp"
  | "rule"
  | "hook"
  | "command"
  | "subagent"
  | "plugin";

export type DriftState =
  | "clean"
  | "local-drift"
  | "upstream-drift"
  | "both-drift"
  | "missing"
  | "orphan";

export interface AgentDto {
  id: string;
  capabilities: string[];
}

export interface InstalledUnitDto {
  agent: string;
  unitType: string;
  name: string;
  paths: string[];
  /** Mirrors InstalledUnitDto.disabled (rig enable / rig disable). */
  disabled?: boolean;
}

export interface DriftReportDto {
  state: DriftState;
  installSha: string | null;
  currentSha: string | null;
  upstreamSha: string | null;
}

export interface UnitBodyDto {
  body: string;
  frontmatter: string;
  path: string;
}

export interface ManifestDto {
  manifest: unknown;
  path: string;
  exists: boolean;
}

export interface LockfileDto {
  lockfile: unknown;
  path: string;
  exists: boolean;
}

export interface ScopeRootsDto {
  globalRig: string;
  home: string;
  claudeGlobal: string;
  codexGlobal: string;
}

export interface InstallResultDto {
  installed: InstalledUnitDto[];
  skipped: string[];
  sourceSha: string;
}

export interface MvResultDto {
  fromScope: Scope;
  toScope: Scope;
  installSha: string;
  disabled: boolean;
}

export interface SyncResultDto {
  installed: InstalledUnitDto[];
  skipped: string[];
  conflicts: string[];
  cancelled: boolean;
}

export interface TypeStatsDto {
  unitType: string;
  count: number;
  bytes: number;
}

export interface AgentStatsDto {
  agent: string;
  byType: TypeStatsDto[];
  totalCount: number;
  totalBytes: number;
}

export interface StatsDto {
  agents: AgentStatsDto[];
  grandTotalCount: number;
  grandTotalBytes: number;
}

export interface DuplicateLocationDto {
  agent: string;
  scope: Scope;
  path: string;
}

export interface DuplicateDto {
  unitType: string;
  name: string;
  locations: DuplicateLocationDto[];
}

export interface DoctorResultDto {
  duplicates: DuplicateDto[];
  brokenSymlinks: string[];
  mvSplit: string[];
  mvStaleLock: string[];
  fixed: number;
}
