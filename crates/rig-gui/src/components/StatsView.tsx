import { useEffect, useState } from "react";
import { statsSummary } from "../lib/api";
import type { Scope, StatsDto } from "../types";

interface Props {
  scope: Scope;
}

function humanBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export default function StatsView({ scope }: Props) {
  const [stats, setStats] = useState<StatsDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setErr(null);
    statsSummary(scope)
      .then((s) => {
        if (live) setStats(s);
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
  }, [scope]);

  if (loading) {
    return <div className="p-4 text-sm text-slate-500">Loading stats…</div>;
  }
  if (err) {
    return <div className="m-4 rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700">{err}</div>;
  }
  if (!stats) {
    return null;
  }

  return (
    <div className="p-4">
      <div className="mb-4 rounded border border-slate-200 bg-slate-50 p-3">
        <div className="text-xs font-semibold uppercase text-slate-500">
          Grand total ({scope})
        </div>
        <div className="text-lg font-semibold">
          {stats.grandTotalCount} units, {humanBytes(stats.grandTotalBytes)}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {stats.agents.map((a) => (
          <div key={a.agent} className="border border-slate-200 rounded p-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-sm font-semibold">{a.agent}</div>
              <div className="text-xs text-slate-500">
                {a.totalCount} units · {humanBytes(a.totalBytes)}
              </div>
            </div>
            {a.byType.length === 0 ? (
              <div className="text-xs text-slate-400">(empty)</div>
            ) : (
              <table className="w-full text-xs">
                <tbody>
                  {a.byType.map((t) => (
                    <tr key={t.unitType} className="border-t border-slate-100">
                      <td className="py-1 font-mono text-slate-700">{t.unitType}</td>
                      <td className="py-1 text-right tabular-nums">{t.count}</td>
                      <td className="py-1 text-right tabular-nums text-slate-500">
                        {humanBytes(t.bytes)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
