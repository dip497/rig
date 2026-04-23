import { useEffect, useState } from "react";
import { statsSummary } from "../lib/api";
import type { Scope, ScopeSelection, StatsDto } from "../types";
import { Card } from "../ui";

interface Props {
  scope: ScopeSelection;
  projectPath?: string;
  hasProject?: boolean;
}

function humanBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function mergeStats(parts: StatsDto[]): StatsDto {
  const byAgent = new Map<
    string,
    { totalCount: number; totalBytes: number; byType: Map<string, { count: number; bytes: number }> }
  >();
  let grandCount = 0;
  let grandBytes = 0;
  for (const p of parts) {
    grandCount += p.grandTotalCount;
    grandBytes += p.grandTotalBytes;
    for (const a of p.agents) {
      let entry = byAgent.get(a.agent);
      if (!entry) {
        entry = { totalCount: 0, totalBytes: 0, byType: new Map() };
        byAgent.set(a.agent, entry);
      }
      entry.totalCount += a.totalCount;
      entry.totalBytes += a.totalBytes;
      for (const t of a.byType) {
        const prev = entry.byType.get(t.unitType) ?? { count: 0, bytes: 0 };
        prev.count += t.count;
        prev.bytes += t.bytes;
        entry.byType.set(t.unitType, prev);
      }
    }
  }
  return {
    grandTotalCount: grandCount,
    grandTotalBytes: grandBytes,
    agents: [...byAgent.entries()].map(([agent, e]) => ({
      agent,
      totalCount: e.totalCount,
      totalBytes: e.totalBytes,
      byType: [...e.byType.entries()].map(([unitType, v]) => ({
        unitType,
        count: v.count,
        bytes: v.bytes,
      })),
    })),
  };
}

export default function StatsView({ scope, projectPath, hasProject }: Props) {
  const [stats, setStats] = useState<StatsDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setErr(null);

    const scopes: Scope[] =
      scope === "all"
        ? hasProject
          ? ["global", "project", "local"]
          : ["global"]
        : (scope === "project" || scope === "local") && !hasProject
          ? []
          : [scope as Scope];

    if (scopes.length === 0) {
      setStats({ agents: [], grandTotalCount: 0, grandTotalBytes: 0 });
      setLoading(false);
      return;
    }

    Promise.all(
      scopes.map((s) =>
        statsSummary(s, s === "global" ? undefined : projectPath),
      ),
    )
      .then((parts) => {
        if (live) setStats(mergeStats(parts));
      })
      .catch((e) => {
        if (live) setErr(String(e));
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [scope, projectPath, hasProject]);

  if (loading) {
    return <div className="p-4 text-sm text-fg-muted">Loading stats…</div>;
  }
  if (err) {
    return (
      <div className="m-4 rounded-md border border-danger/40 bg-danger-subtle p-3 text-sm text-danger-fg">
        {err}
      </div>
    );
  }
  if (!stats) {
    return null;
  }

  return (
    <div className="p-4">
      <Card className="mb-4 bg-surface-2">
        <div className="text-xs font-semibold uppercase text-fg-muted">
          Grand total ({scope})
        </div>
        <div className="text-lg font-semibold text-fg-default">
          {stats.grandTotalCount} units, {humanBytes(stats.grandTotalBytes)}
        </div>
      </Card>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {stats.agents.map((a) => (
          <Card key={a.agent}>
            <div className="mb-2 flex items-center justify-between">
              <div className="text-sm font-semibold text-fg-default">{a.agent}</div>
              <div className="text-xs text-fg-muted">
                {a.totalCount} units · {humanBytes(a.totalBytes)}
              </div>
            </div>
            {a.byType.length === 0 ? (
              <div className="text-xs text-fg-subtle">(empty)</div>
            ) : (
              <table className="w-full text-xs">
                <tbody>
                  {a.byType.map((t) => (
                    <tr key={t.unitType} className="border-t border-border-default">
                      <td className="py-1 font-mono text-fg-default">{t.unitType}</td>
                      <td className="py-1 text-right tabular-nums text-fg-default">{t.count}</td>
                      <td className="py-1 text-right tabular-nums text-fg-muted">
                        {humanBytes(t.bytes)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Card>
        ))}
      </div>
    </div>
  );
}
