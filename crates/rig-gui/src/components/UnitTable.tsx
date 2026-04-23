import type { DriftReportDto, InstalledUnitDto, Scope } from "../types";
import DriftBadge from "./DriftBadge";

export interface UnitRow extends InstalledUnitDto {
  drift?: DriftReportDto | null;
  /** Present when scope="all": which scope this row actually lives in. */
  origin?: Scope;
  /** True when this row is shadowed by a higher-precedence scope. */
  shadowed?: boolean;
}

export type EmptyKind = "no-project" | "no-match";

export default function UnitTable({
  rows,
  onSelect,
  selectedKey,
  showOrigin,
  emptyKind,
  onOpenProject,
  onClearFilters,
}: {
  rows: UnitRow[];
  onSelect: (u: UnitRow) => void;
  selectedKey: string | null;
  showOrigin?: boolean;
  emptyKind?: EmptyKind;
  onOpenProject?: () => void;
  onClearFilters?: () => void;
}) {
  if (rows.length === 0) {
    const isNoProject = emptyKind === "no-project";
    return (
      <div className="flex h-full items-center justify-center px-8 py-16">
        <div className="max-w-md text-center text-slate-500">
          <div className="text-base font-medium text-slate-700">
            {isNoProject
              ? "Open a project to see project-scoped units."
              : "No units match your filters."}
          </div>
          <div className="mt-4">
            {isNoProject ? (
              <button
                onClick={onOpenProject}
                className="rounded bg-indigo-600 px-3 py-1 text-sm text-white shadow-sm hover:bg-indigo-700"
              >
                Open project…
              </button>
            ) : (
              <button
                onClick={onClearFilters}
                className="rounded border border-slate-300 bg-white px-3 py-1 text-sm text-slate-700 shadow-sm hover:bg-slate-50"
              >
                Clear filters
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }
  // NOTE: paths column tooltip listing first few absolute paths could be added
  // via <td title={...}> — skipped for now to keep the change focused.
  return (
    <div className="overflow-auto">
      <table className="w-full text-sm">
        <thead className="sticky top-0 z-10 bg-white text-left text-xs uppercase text-slate-500 shadow-sm">
          <tr>
            <th className="px-3 py-2">Agent</th>
            <th className="px-3 py-2">Type</th>
            <th className="px-3 py-2">Name</th>
            <th className="px-3 py-2">Drift</th>
            <th className="px-3 py-2">Paths</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const k = `${r.agent}/${r.unitType}/${r.name}/${r.origin ?? ""}`;
            return (
              <tr
                key={k}
                onClick={() => onSelect(r)}
                className={`cursor-pointer border-b border-slate-100 hover:bg-slate-50 ${
                  selectedKey === k ? "bg-indigo-50" : ""
                } ${r.disabled || r.shadowed ? "opacity-60" : ""}`}
              >
                <td className="px-3 py-2 font-mono text-xs">{r.agent}</td>
                <td className="px-3 py-2 font-mono text-xs">{r.unitType}</td>
                <td className="px-3 py-2 font-medium">
                  {r.name}
                  {showOrigin && r.origin ? (
                    <span className="ml-2 rounded bg-slate-100 px-1 text-xs text-slate-600">
                      [{r.origin}]
                    </span>
                  ) : null}
                  {r.shadowed ? (
                    <span className="ml-2 text-xs italic text-slate-500">
                      (shadowed by project)
                    </span>
                  ) : null}
                  {r.disabled ? (
                    <span className="ml-2 rounded bg-slate-100 px-1 text-xs text-slate-500">
                      [disabled]
                    </span>
                  ) : null}
                </td>
                <td className="px-3 py-2">
                  <DriftBadge state={r.drift?.state ?? null} />
                </td>
                <td
                  className="px-3 py-2 text-xs text-slate-500"
                  title={r.paths.slice(0, 5).join("\n")}
                >
                  {r.paths.length} file{r.paths.length === 1 ? "" : "s"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
