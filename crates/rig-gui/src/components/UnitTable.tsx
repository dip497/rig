import type { DriftReportDto, InstalledUnitDto, Scope } from "../types";
import { Badge, Button, DriftBadge, EmptyState } from "../ui";

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
      <EmptyState
        title={
          isNoProject
            ? "Open a project to see project-scoped units."
            : "No units match your filters."
        }
        action={
          isNoProject ? (
            <Button variant="primary" size="sm" onClick={onOpenProject}>
              Open project…
            </Button>
          ) : (
            <Button variant="secondary" size="sm" onClick={onClearFilters}>
              Clear filters
            </Button>
          )
        }
      />
    );
  }
  return (
    <div className="overflow-auto">
      <table className="w-full text-sm">
        <thead className="sticky top-0 z-10 bg-surface-1 text-left text-xs uppercase text-fg-muted shadow-sm">
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
                className={`cursor-pointer border-b border-border-default hover:bg-surface-2 ${
                  selectedKey === k ? "bg-accent-subtle" : ""
                } ${r.disabled || r.shadowed ? "opacity-60" : ""}`}
              >
                <td className="px-3 py-2 font-mono text-xs">{r.agent}</td>
                <td className="px-3 py-2 font-mono text-xs">{r.unitType}</td>
                <td className="px-3 py-2 font-medium">
                  {r.name}
                  {showOrigin && r.origin ? (
                    <Badge color="muted" className="ml-2">
                      {r.origin}
                    </Badge>
                  ) : null}
                  {r.shadowed ? (
                    <span className="ml-2 text-xs italic text-fg-subtle">
                      (shadowed by project)
                    </span>
                  ) : null}
                  {r.disabled ? (
                    <Badge color="muted" className="ml-2">
                      disabled
                    </Badge>
                  ) : null}
                </td>
                <td className="px-3 py-2">
                  <DriftBadge state={r.drift?.state ?? null} />
                </td>
                <td
                  className="px-3 py-2 text-xs text-fg-muted"
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
