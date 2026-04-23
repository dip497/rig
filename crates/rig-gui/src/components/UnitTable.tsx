import type { DriftReportDto, InstalledUnitDto, Scope } from "../types";
import DriftBadge from "./DriftBadge";

export interface UnitRow extends InstalledUnitDto {
  drift?: DriftReportDto | null;
  /** Present when scope="all": which scope this row actually lives in. */
  origin?: Scope;
  /** True when this row is shadowed by a higher-precedence scope. */
  shadowed?: boolean;
}

export default function UnitTable({
  rows,
  onSelect,
  selectedKey,
  showOrigin,
}: {
  rows: UnitRow[];
  onSelect: (u: UnitRow) => void;
  selectedKey: string | null;
  showOrigin?: boolean;
}) {
  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-slate-500">
        <div className="text-center">
          <div className="text-lg font-medium">No units found</div>
          <div className="mt-1 text-sm">
            Run <code className="rounded bg-slate-100 px-1">rig install</code> to
            add units.
          </div>
        </div>
      </div>
    );
  }
  return (
    <div className="overflow-auto">
      <table className="w-full text-sm">
        <thead className="sticky top-0 bg-slate-50 text-left text-xs uppercase text-slate-500">
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
                <td className="px-3 py-2 text-xs text-slate-500">
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
