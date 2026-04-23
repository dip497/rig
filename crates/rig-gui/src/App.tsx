import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AgentDto,
  DriftReportDto,
  InstalledUnitDto,
  Scope,
  ScopeSelection,
  UnitTypeId,
} from "./types";
import {
  detectDrift,
  listAgents,
  listUnits,
  searchUnits,
  uninstallUnit,
} from "./lib/api";
import {
  getCurrentProject,
  mergeAcrossScopes,
  setCurrentProject,
  type OriginTaggedUnit,
} from "./lib/project";
import UnitTable, { type UnitRow } from "./components/UnitTable";
import DetailPane from "./components/DetailPane";
import ScopePill from "./components/ScopePill";
import InstallModal from "./components/InstallModal";
import SyncModal from "./components/SyncModal";
import StatsView from "./components/StatsView";
import DoctorView from "./components/DoctorView";
import ProjectPicker from "./components/ProjectPicker";
import TypeFilterPills, {
  PILL_TYPES,
  type TypeFilter,
} from "./components/TypeFilter";
import { Button, Input, Pill, ThemeToggle } from "./ui";

type View = "units" | "stats" | "doctor";

const LS_TYPE_FILTER = "rig-gui.unit-type-filter";
const LS_HIDE_GLOBAL = "rig-gui.hide-global";
const LS_AGENT_FILTER = "rig-gui.agent-filter";

function readLs(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}
function writeLs(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // ignore
  }
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="text-fg-subtle text-[10px] border border-border-default rounded-sm px-1 ml-2">
      {children}
    </kbd>
  );
}

export default function App() {
  const [agents, setAgents] = useState<AgentDto[]>([]);
  const [projectPath, setProjectPathState] = useState<string | null>(
    getCurrentProject(),
  );
  const [scope, setScope] = useState<ScopeSelection>(() =>
    getCurrentProject() ? "all" : "global",
  );
  const [view, setView] = useState<View>("units");
  const [selectedAgent, setSelectedAgent] = useState<string | null>(() => {
    const v = readLs(LS_AGENT_FILTER, "");
    return v === "" ? null : v;
  });
  const [units, setUnits] = useState<OriginTaggedUnit[]>([]);
  const [drifts, setDrifts] = useState<
    Record<string, DriftReportDto | null>
  >({});
  const [selected, setSelected] = useState<UnitRow | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showInstall, setShowInstall] = useState(false);
  const [showSync, setShowSync] = useState(false);
  const [busyUninstall, setBusyUninstall] = useState(false);
  const [query, setQuery] = useState("");
  const [banner, setBanner] = useState<string | null>(null);
  const [typeFilter, setTypeFilter] = useState<TypeFilter>(() => {
    const v = readLs(LS_TYPE_FILTER, "all") as TypeFilter;
    return PILL_TYPES.includes(v) ? v : "all";
  });
  const [hideGlobal, setHideGlobal] = useState<boolean>(
    () => readLs(LS_HIDE_GLOBAL, "false") === "true",
  );
  const searchRef = useRef<HTMLInputElement | null>(null);

  const setProjectPath = useCallback((p: string | null) => {
    setCurrentProject(p);
    setProjectPathState(p);
    if (!p) {
      setScope((cur) => (cur === "global" ? cur : "global"));
    }
  }, []);

  const hasProject = !!projectPath;
  const needsProjectBanner =
    (scope === "project" || scope === "local" || scope === "all") && !hasProject
      ? "Open a project to view project-scoped units."
      : null;

  const scopesToQuery: Scope[] = useMemo(() => {
    if (scope === "all") {
      if (!hasProject) return ["global"];
      return ["global", "project", "local"];
    }
    if ((scope === "project" || scope === "local") && !hasProject) return [];
    return [scope];
  }, [scope, hasProject]);

  useEffect(() => {
    if (view !== "units") return;
    let cancelled = false;
    const trimmed = query.trim();
    const handle = window.setTimeout(
      async () => {
        try {
          if (scopesToQuery.length === 0) {
            if (!cancelled) setUnits([]);
            return;
          }
          const perScope = await Promise.all(
            scopesToQuery.map(async (s) => {
              const pp = s === "global" ? undefined : projectPath ?? undefined;
              const us: InstalledUnitDto[] = trimmed
                ? await searchUnits(s, trimmed, pp)
                : await listUnits(s, pp);
              return { scope: s, units: us };
            }),
          );
          const merged = mergeAcrossScopes(perScope);
          if (!cancelled) setUnits(merged);
        } catch (e) {
          if (!cancelled) setError(String(e));
        }
      },
      trimmed ? 200 : 0,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [query, scope, view, projectPath, scopesToQuery]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const ag = await listAgents();
      setAgents(ag);
      if (scopesToQuery.length === 0) {
        setUnits([]);
        setDrifts({});
        return;
      }
      const trimmed = query.trim();
      const perScope = await Promise.all(
        scopesToQuery.map(async (s) => {
          const pp = s === "global" ? undefined : projectPath ?? undefined;
          const us: InstalledUnitDto[] = trimmed
            ? await searchUnits(s, trimmed, pp)
            : await listUnits(s, pp);
          return { scope: s, units: us };
        }),
      );
      const merged = mergeAcrossScopes(perScope);
      setUnits(merged);
      const d: Record<string, DriftReportDto | null> = {};
      await Promise.all(
        merged.map(async (u) => {
          const k = `${u.agent}/${u.unitType}/${u.name}/${u.origin}`;
          try {
            const pp = u.origin === "global" ? undefined : projectPath ?? undefined;
            d[k] = await detectDrift(
              u.origin,
              u.agent,
              u.unitType as UnitTypeId,
              u.name,
              pp,
            );
          } catch {
            d[k] = null;
          }
        }),
      );
      setDrifts(d);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [scopesToQuery, projectPath, query]);

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope, projectPath]);

  useEffect(() => {
    writeLs(LS_TYPE_FILTER, typeFilter);
  }, [typeFilter]);
  useEffect(() => {
    writeLs(LS_HIDE_GLOBAL, hideGlobal ? "true" : "false");
  }, [hideGlobal]);
  useEffect(() => {
    writeLs(LS_AGENT_FILTER, selectedAgent ?? "");
  }, [selectedAgent]);

  useEffect(() => {
    if (!banner) return;
    const h = window.setTimeout(() => setBanner(null), 3000);
    return () => window.clearTimeout(h);
  }, [banner]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const cmd = e.metaKey || e.ctrlKey;
      if (cmd && e.key.toLowerCase() === "r") {
        e.preventDefault();
        refresh();
        return;
      }
      if (cmd && e.key.toLowerCase() === "k" && view === "units") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
        return;
      }
      if (cmd && e.key === "1") {
        e.preventDefault();
        setView("units");
      } else if (cmd && e.key === "2") {
        e.preventDefault();
        setView("stats");
      } else if (cmd && e.key === "3") {
        e.preventDefault();
        setView("doctor");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh, view]);

  const filteredUnits = useMemo(() => {
    return units.filter((u) => {
      if (selectedAgent && u.agent !== selectedAgent) return false;
      if (typeFilter !== "all" && u.unitType !== typeFilter) return false;
      if (scope === "all" && hideGlobal && u.origin === "global") return false;
      return true;
    });
  }, [units, selectedAgent, typeFilter, scope, hideGlobal]);

  const typeCounts = useMemo(() => {
    const pool = units.filter((u) => {
      if (selectedAgent && u.agent !== selectedAgent) return false;
      if (scope === "all" && hideGlobal && u.origin === "global") return false;
      return true;
    });
    const counts: Record<TypeFilter, number> = {
      all: pool.length,
      skill: 0,
      mcp: 0,
      rule: 0,
      command: 0,
      subagent: 0,
    };
    for (const u of pool) {
      const t = u.unitType as TypeFilter;
      if (t in counts && t !== "all") counts[t] = (counts[t] ?? 0) + 1;
    }
    return counts;
  }, [units, selectedAgent, scope, hideGlobal]);

  const rows: UnitRow[] = useMemo(
    () =>
      filteredUnits.map((u) => ({
        ...u,
        drift: drifts[`${u.agent}/${u.unitType}/${u.name}/${u.origin}`] ?? null,
      })),
    [filteredUnits, drifts],
  );

  const selectedKey = selected
    ? `${selected.agent}/${selected.unitType}/${selected.name}/${selected.origin ?? ""}`
    : null;

  const effectiveScope: Scope =
    scope === "all"
      ? hasProject
        ? "project"
        : "global"
      : (scope as Scope);

  const openProjectPicker = useCallback(async () => {
    try {
      const result = await openDialog({
        directory: true,
        multiple: false,
        title: "Open project",
      });
      if (typeof result === "string") setProjectPath(result);
    } catch (e) {
      console.error("open project failed:", e);
    }
  }, [setProjectPath]);

  const clearFilters = useCallback(() => {
    setTypeFilter("all");
    setSelectedAgent(null);
    setQuery("");
  }, []);

  const emptyKind =
    scope !== "global" && !hasProject ? "no-project" : "no-match";

  return (
    <div className="flex h-screen flex-col bg-bg-canvas text-fg-default">
      {/* Row 1 — app-level */}
      <header className="flex items-center justify-between border-b border-border-default bg-surface-1 px-4 py-2">
        <div className="flex items-center gap-3">
          <div className="text-lg font-bold tracking-tight">Rig</div>
          <ProjectPicker current={projectPath} onPick={setProjectPath} />
        </div>
        <div className="flex items-center gap-2">
          <ThemeToggle />
          <Button variant="secondary" size="sm" onClick={refresh}>
            {loading ? "Refreshing…" : "Refresh"}
            <Kbd>⌘R</Kbd>
          </Button>
        </div>
      </header>

      {/* Tabs row */}
      <div className="flex items-center gap-1 border-b border-border-default bg-surface-1 px-4 py-1.5">
        <Pill active={view === "units"} onClick={() => setView("units")}>
          Units
        </Pill>
        <Pill active={view === "stats"} onClick={() => setView("stats")}>
          Stats
        </Pill>
        <Pill active={view === "doctor"} onClick={() => setView("doctor")}>
          Doctor
        </Pill>
      </div>

      {/* Row 2 — context / view-level (Units only) */}
      {view === "units" && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border-default bg-surface-2 px-4 py-2">
          <div className="flex flex-wrap items-center gap-3">
            <TypeFilterPills
              selected={typeFilter}
              counts={typeCounts}
              onChange={setTypeFilter}
            />
            <div className="h-5 w-px bg-border-default" />
            <div className="flex items-center gap-1">
              <Pill
                active={selectedAgent === null}
                onClick={() => setSelectedAgent(null)}
              >
                All
              </Pill>
              {agents.map((a) => (
                <Pill
                  key={a.id}
                  active={selectedAgent === a.id}
                  onClick={() => setSelectedAgent(a.id)}
                >
                  {a.id}
                </Pill>
              ))}
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <ScopePill scope={scope} onChange={setScope} hasProject={hasProject} />
            {scope === "all" && hasProject && (
              <label className="flex items-center gap-1 text-xs text-fg-muted">
                <input
                  type="checkbox"
                  checked={hideGlobal}
                  onChange={(e) => setHideGlobal(e.target.checked)}
                />
                Hide global
              </label>
            )}
            <div className="flex items-center">
              <Input
                ref={searchRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search"
                className="w-48"
              />
              <Kbd>⌘K</Kbd>
            </div>
            <Button variant="secondary" size="sm" onClick={() => setShowSync(true)}>
              Sync
            </Button>
            <Button variant="primary" size="sm" onClick={() => setShowInstall(true)}>
              + Install
            </Button>
          </div>
        </div>
      )}

      {needsProjectBanner && (
        <div className="border-b border-warning/40 bg-warning-subtle px-4 py-2 text-sm text-warning-fg">
          {needsProjectBanner}
        </div>
      )}

      {banner && (
        <div
          className="border-b border-success/40 bg-success-subtle px-4 py-2 text-sm text-success-fg cursor-pointer"
          onClick={() => setBanner(null)}
        >
          {banner}
        </div>
      )}

      {showInstall && (
        <InstallModal
          agents={agents}
          scope={effectiveScope}
          projectPath={projectPath ?? undefined}
          onClose={() => setShowInstall(false)}
          onInstalled={refresh}
        />
      )}

      {showSync && (
        <SyncModal
          scope={effectiveScope}
          projectPath={projectPath ?? undefined}
          onClose={() => setShowSync(false)}
          onDone={() => {
            setBanner("Sync complete.");
            refresh();
          }}
        />
      )}

      {error && (
        <div className="border-b border-danger/40 bg-danger-subtle px-4 py-2 text-sm text-danger-fg">
          {error}
        </div>
      )}

      {view === "units" && (
        <div className="flex flex-1 overflow-hidden">
          <main className="flex-1 overflow-auto bg-surface-1">
            <UnitTable
              rows={rows}
              onSelect={setSelected}
              selectedKey={selectedKey}
              showOrigin={scope === "all"}
              emptyKind={emptyKind}
              onOpenProject={openProjectPicker}
              onClearFilters={clearFilters}
            />
          </main>
          {selected && (
            <DetailPane
              agent={selected.agent}
              unitType={selected.unitType}
              name={selected.name}
              paths={selected.paths}
              scope={selected.origin ?? effectiveScope}
              projectPath={projectPath ?? undefined}
              drift={selected.drift}
              disabled={selected.disabled}
              busy={busyUninstall}
              onChanged={async () => {
                setSelected(null);
                await refresh();
              }}
              onUninstall={async () => {
                if (
                  !confirm(
                    `Uninstall ${selected.unitType}/${selected.name} from ${selected.agent}?`,
                  )
                )
                  return;
                setBusyUninstall(true);
                setError(null);
                try {
                  const unitScope = selected.origin ?? effectiveScope;
                  const pp =
                    unitScope === "global" ? undefined : projectPath ?? undefined;
                  await uninstallUnit(
                    unitScope,
                    selected.agent,
                    selected.unitType as UnitTypeId,
                    selected.name,
                    pp,
                  );
                  setSelected(null);
                  await refresh();
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusyUninstall(false);
                }
              }}
            />
          )}
        </div>
      )}

      {view === "stats" && (
        <main className="flex-1 overflow-auto bg-surface-1">
          <StatsView
            scope={scope}
            projectPath={projectPath ?? undefined}
            hasProject={hasProject}
          />
        </main>
      )}

      {view === "doctor" && (
        <main className="flex-1 overflow-auto bg-surface-1">
          <DoctorView
            scope={scope}
            projectPath={projectPath ?? undefined}
            hasProject={hasProject}
          />
        </main>
      )}
    </div>
  );
}
