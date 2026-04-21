import { invoke } from "@tauri-apps/api/core";
import type {
  AgentDto,
  DriftReportDto,
  InstallResultDto,
  InstalledUnitDto,
  LockfileDto,
  ManifestDto,
  Scope,
  ScopeRootsDto,
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
