import { useEffect, useState } from "react";
import type { DriftReportDto, Scope, UnitBodyDto, UnitTypeId } from "../types";
import { mvUnit, readUnitBody, setEnabled } from "../lib/api";
import { shortSha } from "../lib/format";
import DriftBadge from "./DriftBadge";

interface Props {
  agent: string;
  unitType: string;
  name: string;
  paths: string[];
  scope: Scope;
  drift: DriftReportDto | null | undefined;
  disabled?: boolean;
  onUninstall?: () => void;
  onChanged?: () => void;
  busy?: boolean;
}

const ALL_SCOPES: Scope[] = ["global", "project", "local"];

export default function DetailPane({
  agent,
  unitType,
  name,
  paths,
  scope,
  drift,
  disabled,
  onUninstall,
  onChanged,
  busy,
}: Props) {
  const [body, setBody] = useState<UnitBodyDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [moveTo, setMoveTo] = useState<Scope>(
    (ALL_SCOPES.find((s) => s !== scope) ?? "project") as Scope,
  );

  useEffect(() => {
    setBody(null);
    setErr(null);
    readUnitBody(scope, agent, unitType as UnitTypeId, name)
      .then(setBody)
      .catch((e) => setErr(String(e)));
  }, [agent, unitType, name, scope]);

  useEffect(() => {
    // Keep move-dest default off the current scope when scope changes.
    if (moveTo === scope) {
      setMoveTo((ALL_SCOPES.find((s) => s !== scope) ?? "project") as Scope);
    }
  }, [scope, moveTo]);

  const doToggle = async () => {
    setErr(null);
    setActionBusy(true);
    try {
      // New enabled state is the inverse of "is currently enabled".
      // is-currently-enabled == !disabled, so new state == disabled.
      const newEnabled = !!disabled;
      await setEnabled(
        scope,
        agent,
        unitType as UnitTypeId,
        name,
        newEnabled,
      );
      onChanged?.();
    } catch (e) {
      const msg = String(e);
      if (msg.toLowerCase().includes("unsupported")) {
        setErr("Enable/disable not supported for this unit type");
      } else {
        setErr(msg);
      }
    } finally {
      setActionBusy(false);
    }
  };

  const doMove = async () => {
    if (moveTo === scope) return;
    setErr(null);
    setActionBusy(true);
    try {
      await mvUnit(scope, moveTo, agent, unitType as UnitTypeId, name);
      onChanged?.();
    } catch (e) {
      const msg = String(e);
      if (msg.toLowerCase().includes("unsupported")) {
        setErr(`Move not supported: ${msg}`);
      } else {
        setErr(msg);
      }
    } finally {
      setActionBusy(false);
    }
  };

  const anyBusy = busy || actionBusy;

  return (
    <aside className="w-[420px] overflow-auto border-l border-slate-200 bg-white p-4">
      <div className="mb-3">
        <div className="text-xs uppercase text-slate-500">
          {agent} · {unitType}
        </div>
        <h2 className="text-lg font-semibold">
          {name}
          {disabled ? (
            <span className="ml-2 rounded bg-slate-100 px-1 text-xs text-slate-500">
              [disabled]
            </span>
          ) : null}
        </h2>
        <div className="mt-2">
          <DriftBadge state={drift?.state ?? null} />
        </div>
      </div>

      <div className="mb-3 flex items-center gap-2">
        <button
          onClick={doToggle}
          disabled={anyBusy}
          className="rounded border border-slate-300 bg-white px-2 py-0.5 text-xs text-slate-700 shadow-sm hover:bg-slate-50 disabled:opacity-50"
        >
          {disabled ? "Enable" : "Disable"}
        </button>
        <div className="flex items-center gap-1">
          <select
            value={moveTo}
            onChange={(e) => setMoveTo(e.target.value as Scope)}
            disabled={anyBusy}
            className="rounded border border-slate-300 bg-white px-1 py-0.5 text-xs text-slate-700 shadow-sm disabled:opacity-50"
          >
            {ALL_SCOPES.map((s) => (
              <option key={s} value={s} disabled={s === scope}>
                {s}
                {s === scope ? " (current)" : ""}
              </option>
            ))}
          </select>
          <button
            onClick={doMove}
            disabled={anyBusy || moveTo === scope}
            className="rounded border border-slate-300 bg-white px-2 py-0.5 text-xs text-slate-700 shadow-sm hover:bg-slate-50 disabled:opacity-50"
          >
            Move to…
          </button>
        </div>
        {onUninstall && (
          <button
            onClick={onUninstall}
            disabled={anyBusy}
            className="ml-auto rounded border border-red-300 bg-white px-2 py-0.5 text-xs text-red-700 shadow-sm hover:bg-red-50 disabled:opacity-50"
          >
            {busy ? "Removing…" : "Uninstall"}
          </button>
        )}
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
        <div className="mb-3 rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
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
