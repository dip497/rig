import { invoke } from "@tauri-apps/api/core";
import type {
  AgentDto,
  DoctorResultDto,
  DriftReportDto,
  InstallResultDto,
  InstalledUnitDto,
  LockfileDto,
  ManifestDto,
  MvResultDto,
  Scope,
  ScopeRootsDto,
  StatsDto,
  SyncResultDto,
  UnitBodyDto,
  UnitTypeId,
} from "../types";

export const listAgents = () => invoke<AgentDto[]>("list_agents");

export const listUnits = (scope: Scope, projectPath?: string) =>
  invoke<InstalledUnitDto[]>("list_units", { scope, projectPath });

export const detectDrift = (
  scope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  projectPath?: string,
) =>
  invoke<DriftReportDto>("detect_drift", {
    scope,
    projectPath,
    agent,
    unitType,
    name,
  });

export const readUnitBody = (
  scope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  projectPath?: string,
) =>
  invoke<UnitBodyDto>("read_unit_body", {
    scope,
    projectPath,
    agent,
    unitType,
    name,
  });

export const readManifest = (scope: Scope, projectPath?: string) =>
  invoke<ManifestDto>("read_manifest", { scope, projectPath });

export const readLockfile = (scope: Scope, projectPath?: string) =>
  invoke<LockfileDto>("read_lockfile", { scope, projectPath });

export const scopeRoots = () => invoke<ScopeRootsDto>("scope_roots");

export const installUnit = (params: {
  scope: Scope;
  source: string;
  agents: string[];
  asType?: UnitTypeId;
  projectPath?: string;
}) => invoke<InstallResultDto>("install_unit", params);

export const uninstallUnit = (
  scope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  projectPath?: string,
) =>
  invoke<void>("uninstall_unit", {
    scope,
    projectPath,
    agent,
    unitType,
    name,
  });

export const setEnabled = (
  scope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  enabled: boolean,
  projectPath?: string,
) =>
  invoke<void>("set_enabled", {
    scope,
    projectPath,
    agent,
    unitType,
    name,
    enabled,
  });

export const isEnabled = (
  scope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  projectPath?: string,
) =>
  invoke<boolean>("is_enabled", {
    scope,
    projectPath,
    agent,
    unitType,
    name,
  });

export const syncScope = (scope: Scope, onDrift: string, projectPath?: string) =>
  invoke<SyncResultDto>("sync_scope", { scope, onDrift, projectPath });

export const searchUnits = (scope: Scope, query: string, projectPath?: string) =>
  invoke<InstalledUnitDto[]>("search_units", { scope, query, projectPath });

export const statsSummary = (scope: Scope, projectPath?: string) =>
  invoke<StatsDto>("stats_summary", { scope, projectPath });

export const doctorScan = (scope: Scope, fix: boolean, projectPath?: string) =>
  invoke<DoctorResultDto>("doctor_scan", { scope, fix, projectPath });

export const mvUnit = (
  fromScope: Scope,
  toScope: Scope,
  agent: string,
  unitType: UnitTypeId,
  name: string,
  projectPath?: string,
) =>
  invoke<MvResultDto>("mv_unit", {
    fromScope,
    toScope,
    projectPath,
    agent,
    unitType,
    name,
  });
