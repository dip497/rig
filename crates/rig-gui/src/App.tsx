import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  AgentDto,
  DriftReportDto,
  InstalledUnitDto,
  Scope,
  UnitTypeId,
} from "./types";
import { detectDrift, listAgents, listUnits } from "./lib/api";
import Sidebar from "./components/Sidebar";
import UnitTable, { type UnitRow } from "./components/UnitTable";
import DetailPane from "./components/DetailPane";
import ScopePill from "./components/ScopePill";

export default function App() {
  const [agents, setAgents] = useState<AgentDto[]>([]);
  const [scope, setScope] = useState<Scope>("global");
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [units, setUnits] = useState<InstalledUnitDto[]>([]);
  const [drifts, setDrifts] = useState<
    Record<string, DriftReportDto | null>
  >({});
  const [selected, setSelected] = useState<UnitRow | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [ag, us] = await Promise.all([listAgents(), listUnits(scope)]);
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
  }, [scope]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "r") {
        e.preventDefault();
        refresh();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh]);

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

  return (
    <div className="flex h-screen flex-col">
      <header className="flex items-center justify-between border-b border-slate-200 bg-white px-4 py-2">
        <div className="flex items-center gap-3">
          <div className="text-lg font-bold tracking-tight">Rig</div>
          <span className="text-xs text-slate-500">
            cross-agent package manager
          </span>
        </div>
        <div className="flex items-center gap-3">
          <ScopePill scope={scope} onChange={setScope} />
          <button
            onClick={refresh}
            className="rounded border border-slate-300 bg-white px-3 py-1 text-sm shadow-sm hover:bg-slate-50"
          >
            {loading ? "Refreshing…" : "Refresh (⌘R)"}
          </button>
        </div>
      </header>

      {error && (
        <div className="border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-700">
          {error}
        </div>
      )}

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
          />
        )}
      </div>
    </div>
  );
}
