import { useCallback, useEffect, useState } from "react";
import { doctorScan } from "../lib/api";
import type { DoctorResultDto, Scope } from "../types";

interface Props {
  scope: Scope;
}

export default function DoctorView({ scope }: Props) {
  const [res, setRes] = useState<DoctorResultDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [fixing, setFixing] = useState(false);

  const scan = useCallback(
    async (fix: boolean) => {
      if (fix) setFixing(true);
      else setLoading(true);
      setErr(null);
      try {
        const r = await doctorScan(scope, fix);
        setRes(r);
      } catch (e) {
        setErr(String(e));
      } finally {
        setLoading(false);
        setFixing(false);
      }
    },
    [scope],
  );

  useEffect(() => {
    scan(false);
  }, [scan]);

  if (loading) {
    return <div className="p-4 text-sm text-slate-500">Scanning…</div>;
  }
  if (err) {
    return (
      <div className="m-4 rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700">
        {err}
      </div>
    );
  }
  if (!res) return null;

  const canFix = res.mvStaleLock.length > 0;
  const nothingWrong =
    res.duplicates.length === 0 &&
    res.brokenSymlinks.length === 0 &&
    res.mvSplit.length === 0 &&
    res.mvStaleLock.length === 0;

  return (
    <div className="p-4">
      <div className="mb-4 flex items-center justify-between">
        <div className="text-sm text-slate-600">
          Scope: <span className="font-mono">{scope}</span>
          {res.fixed > 0 && (
            <span className="ml-3 rounded bg-green-100 px-2 py-0.5 text-xs text-green-800">
              fixed {res.fixed}
            </span>
          )}
        </div>
        <button
          onClick={() => scan(true)}
          disabled={!canFix || fixing}
          className="rounded bg-indigo-600 px-3 py-1 text-sm text-white shadow-sm hover:bg-indigo-700 disabled:opacity-40"
        >
          {fixing ? "Fixing…" : "Fix auto-reconcilable issues"}
        </button>
      </div>

      {nothingWrong && (
        <div className="rounded border border-green-200 bg-green-50 p-3 text-sm text-green-800">
          All clean.
        </div>
      )}

      {res.duplicates.length > 0 && (
        <Section title={`Duplicates (${res.duplicates.length})`}>
          {res.duplicates.map((d) => (
            <div
              key={`${d.unitType}/${d.name}`}
              className="border border-slate-200 rounded p-2 mb-2"
            >
              <div className="font-mono text-sm">
                {d.unitType}/{d.name}
              </div>
              <ul className="mt-1 text-xs text-slate-600">
                {d.locations.map((l, i) => (
                  <li key={i} className="font-mono">
                    [{l.agent}] ({l.scope}) {l.path}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </Section>
      )}

      {res.brokenSymlinks.length > 0 && (
        <Section title={`Broken symlinks (${res.brokenSymlinks.length})`}>
          <ul className="font-mono text-xs text-slate-700">
            {res.brokenSymlinks.map((s, i) => (
              <li key={i} className="border-t border-slate-100 py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {res.mvSplit.length > 0 && (
        <Section title={`Mv split-state (${res.mvSplit.length})`}>
          <ul className="font-mono text-xs text-slate-700">
            {res.mvSplit.map((s, i) => (
              <li key={i} className="border-t border-slate-100 py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {res.mvStaleLock.length > 0 && (
        <Section title={`Stale lockfile entries (${res.mvStaleLock.length})`}>
          <ul className="font-mono text-xs text-slate-700">
            {res.mvStaleLock.map((s, i) => (
              <li key={i} className="border-t border-slate-100 py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-4">
      <h3 className="mb-2 text-sm font-semibold text-slate-800">{title}</h3>
      {children}
    </div>
  );
}
