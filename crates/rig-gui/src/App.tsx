import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  AgentDto,
  DriftReportDto,
  InstalledUnitDto,
  Scope,
  UnitTypeId,
} from "./types";
import {
  detectDrift,
  listAgents,
  listUnits,
  searchUnits,
  uninstallUnit,
} from "./lib/api";
import Sidebar from "./components/Sidebar";
import UnitTable, { type UnitRow } from "./components/UnitTable";
import DetailPane from "./components/DetailPane";
import ScopePill from "./components/ScopePill";
import InstallModal from "./components/InstallModal";
import SyncModal from "./components/SyncModal";
import StatsView from "./components/StatsView";
import DoctorView from "./components/DoctorView";

type View = "units" | "stats" | "doctor";

export default function App() {
  const [agents, setAgents] = useState<AgentDto[]>([]);
  const [scope, setScope] = useState<Scope>("global");
  const [view, setView] = useState<View>("units");
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [units, setUnits] = useState<InstalledUnitDto[]>([]);
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
  const searchRef = useRef<HTMLInputElement | null>(null);

  // Debounced search effect.
  useEffect(() => {
    if (view !== "units") return;
    let cancelled = false;
    const trimmed = query.trim();
    const handle = window.setTimeout(
      async () => {
        try {
          const us = trimmed
            ? await searchUnits(scope, trimmed)
            : await listUnits(scope);
          if (!cancelled) setUnits(us);
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
  }, [query, scope, view]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [ag, us] = await Promise.all([
        listAgents(),
        query.trim() ? searchUnits(scope, query.trim()) : listUnits(scope),
      ]);
      setAgents(ag);
      setUnits(us);
      const d: Record<string, DriftReportDto | null> = {};
      await Promise.all(
        us.map(async (u) => {
          const k = `${u.agent}/${u.unitType}/${u.name}`;
          try {
            d[k] = await detectDrift(
              scope,
              u.agent,
              u.unitType as UnitTypeId,
              u.name,
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
  }, [scope, query]);

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope]);

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

  const rows: UnitRow[] = useMemo(
    () =>
      units
        .filter((u) => !selectedAgent || u.agent === selectedAgent)
        .map((u) => ({
          ...u,
          drift: drifts[`${u.agent}/${u.unitType}/${u.name}`] ?? null,
        })),
    [units, selectedAgent, drifts],
  );

  const selectedKey = selected
    ? `${selected.agent}/${selected.unitType}/${selected.name}`
    : null;

  const tabClass = (v: View) =>
    `px-3 py-1 rounded text-sm ${view === v ? "bg-slate-900 text-white" : "hover:bg-slate-100 text-slate-700"}`;

  return (
    <div className="flex h-screen flex-col">
      <header className="flex items-center justify-between border-b border-slate-200 bg-white px-4 py-2">
        <div className="flex items-center gap-3">
          <div className="text-lg font-bold tracking-tight">Rig</div>
          <nav className="flex items-center gap-1">
            <button className={tabClass("units")} onClick={() => setView("units")}>
              Units
            </button>
            <button className={tabClass("stats")} onClick={() => setView("stats")}>
              Stats
            </button>
            <button className={tabClass("doctor")} onClick={() => setView("doctor")}>
              Doctor
            </button>
          </nav>
        </div>
        <div className="flex items-center gap-3">
          <ScopePill scope={scope} onChange={setScope} />
          {view === "units" && (
            <>
              <input
                ref={searchRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search (⌘K)"
                className="rounded border border-slate-300 px-2 py-1 text-sm w-48"
              />
              <button
                onClick={() => setShowSync(true)}
                className="rounded border border-slate-300 bg-white px-3 py-1 text-sm shadow-sm hover:bg-slate-50"
              >
                Sync
              </button>
              <button
                onClick={() => setShowInstall(true)}
                className="rounded bg-indigo-600 px-3 py-1 text-sm text-white shadow-sm hover:bg-indigo-700"
              >
                + Install
              </button>
              <button
                onClick={refresh}
                className="rounded border border-slate-300 bg-white px-3 py-1 text-sm shadow-sm hover:bg-slate-50"
              >
                {loading ? "Refreshing…" : "Refresh (⌘R)"}
              </button>
            </>
          )}
        </div>
      </header>

      {banner && (
        <div className="border-b border-green-200 bg-green-50 px-4 py-2 text-sm text-green-800 cursor-pointer"
             onClick={() => setBanner(null)}>
          {banner}
        </div>
      )}

      {showInstall && (
        <InstallModal
          agents={agents}
          scope={scope}
          onClose={() => setShowInstall(false)}
          onInstalled={refresh}
        />
      )}

      {showSync && (
        <SyncModal
          scope={scope}
          onClose={() => setShowSync(false)}
          onDone={() => {
            setBanner("Sync complete.");
            refresh();
          }}
        />
      )}

      {error && (
        <div className="border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-700">
          {error}
        </div>
      )}

      {view === "units" && (
        <div className="flex flex-1 overflow-hidden">
          <Sidebar
            agents={agents}
            selected={selectedAgent}
            onSelect={setSelectedAgent}
          />
          <main className="flex-1 overflow-auto bg-white">
            <UnitTable
              rows={rows}
              onSelect={setSelected}
              selectedKey={selectedKey}
            />
          </main>
          {selected && (
            <DetailPane
              agent={selected.agent}
              unitType={selected.unitType}
              name={selected.name}
              paths={selected.paths}
              scope={scope}
              drift={selected.drift}
              disabled={selected.disabled}
              busy={busyUninstall}
              onChanged={async () => {
                setSelected(null);
                await refresh();
              }}
              onUninstall={async () => {
                if (!confirm(`Uninstall ${selected.unitType}/${selected.name} from ${selected.agent}?`)) return;
                setBusyUninstall(true);
                setError(null);
                try {
                  await uninstallUnit(
                    scope,
                    selected.agent,
                    selected.unitType as UnitTypeId,
                    selected.name,
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
        <main className="flex-1 overflow-auto bg-white">
          <StatsView scope={scope} />
        </main>
      )}

      {view === "doctor" && (
        <main className="flex-1 overflow-auto bg-white">
          <DoctorView scope={scope} />
        </main>
      )}
    </div>
  );
}
