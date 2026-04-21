import { useEffect, useState } from "react";
import type { DriftReportDto, Scope, UnitBodyDto, UnitTypeId } from "../types";
import { readUnitBody } from "../lib/api";
import { shortSha } from "../lib/format";
import DriftBadge from "./DriftBadge";

interface Props {
  agent: string;
  unitType: string;
  name: string;
  paths: string[];
  scope: Scope;
  drift: DriftReportDto | null | undefined;
  onUninstall?: () => void;
  busy?: boolean;
}

export default function DetailPane({
  agent,
  unitType,
  name,
  paths,
  scope,
  drift,
  onUninstall,
  busy,
}: Props) {
  const [body, setBody] = useState<UnitBodyDto | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setBody(null);
    setErr(null);
    readUnitBody(scope, agent, unitType as UnitTypeId, name)
      .then(setBody)
      .catch((e) => setErr(String(e)));
  }, [agent, unitType, name, scope]);

  return (
    <aside className="w-[420px] overflow-auto border-l border-slate-200 bg-white p-4">
      <div className="mb-3">
        <div className="text-xs uppercase text-slate-500">
          {agent} · {unitType}
        </div>
        <h2 className="text-lg font-semibold">{name}</h2>
        <div className="mt-2 flex items-center justify-between gap-2">
          <DriftBadge state={drift?.state ?? null} />
          {onUninstall && (
            <button
              onClick={onUninstall}
              disabled={busy}
              className="rounded border border-red-300 bg-white px-2 py-0.5 text-xs text-red-700 shadow-sm hover:bg-red-50 disabled:opacity-50"
            >
              {busy ? "Removing…" : "Uninstall"}
            </button>
          )}
        </div>
      </div>

      <div className="mb-3 rounded border border-slate-200 bg-slate-50 p-2 text-xs">
        <div className="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-1 font-mono">
          <span className="text-slate-500">install</span>
          <span>{shortSha(drift?.installSha)}</span>
          <span className="text-slate-500">disk</span>
          <span>{shortSha(drift?.currentSha)}</span>
          <span className="text-slate-500">upstream</span>
          <span>{shortSha(drift?.upstreamSha)}</span>
        </div>
      </div>

      <div className="mb-3">
        <div className="mb-1 text-xs font-semibold uppercase text-slate-500">
          Paths
        </div>
        <ul className="space-y-0.5 text-xs font-mono text-slate-700">
          {paths.map((p) => (
            <li key={p} className="truncate">
              {p}
            </li>
          ))}
        </ul>
      </div>

      {err && (
        <div className="rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
          {err}
        </div>
      )}

      {body && body.frontmatter && (
        <div className="mb-3">
          <div className="mb-1 text-xs font-semibold uppercase text-slate-500">
            Frontmatter
          </div>
          <pre className="whitespace-pre-wrap rounded border border-slate-200 bg-slate-50 p-2 font-mono text-xs">
            {body.frontmatter}
          </pre>
        </div>
      )}

      {body && (
        <div>
          <div className="mb-1 text-xs font-semibold uppercase text-slate-500">
            Body
          </div>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap rounded border border-slate-200 bg-slate-50 p-2 font-mono text-xs">
            {body.body || "(empty)"}
          </pre>
        </div>
      )}
    </aside>
  );
}
