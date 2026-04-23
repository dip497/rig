import { useEffect, useState } from "react";
import type { DriftReportDto, Scope, UnitBodyDto, UnitTypeId } from "../types";
import { mvUnit, readUnitBody, setEnabled } from "../lib/api";
import { shortSha } from "../lib/format";
import { Badge, Button, DriftBadge } from "../ui";

interface Props {
  agent: string;
  unitType: string;
  name: string;
  paths: string[];
  scope: Scope;
  projectPath?: string;
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
  projectPath,
  drift,
  disabled,
  onUninstall,
  onChanged,
  busy,
}: Props) {
  const ppFor = (s: Scope) => (s === "global" ? undefined : projectPath);
  const [body, setBody] = useState<UnitBodyDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [moveTo, setMoveTo] = useState<Scope>(
    (ALL_SCOPES.find((s) => s !== scope) ?? "project") as Scope,
  );

  useEffect(() => {
    setBody(null);
    setErr(null);
    readUnitBody(scope, agent, unitType as UnitTypeId, name, ppFor(scope))
      .then(setBody)
      .catch((e) => setErr(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [agent, unitType, name, scope, projectPath]);

  useEffect(() => {
    if (moveTo === scope) {
      setMoveTo((ALL_SCOPES.find((s) => s !== scope) ?? "project") as Scope);
    }
  }, [scope, moveTo]);

  const doToggle = async () => {
    setErr(null);
    setActionBusy(true);
    try {
      const newEnabled = !!disabled;
      await setEnabled(
        scope,
        agent,
        unitType as UnitTypeId,
        name,
        newEnabled,
        ppFor(scope),
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
      const pp = scope === "global" && moveTo === "global" ? undefined : projectPath;
      await mvUnit(scope, moveTo, agent, unitType as UnitTypeId, name, pp);
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
    <aside className="w-[420px] overflow-auto border-l border-border-default bg-surface-1 p-4">
      <div className="mb-3">
        <div className="text-xs uppercase text-fg-muted">
          {agent} · {unitType}
        </div>
        <h2 className="text-lg font-semibold text-fg-default">
          {name}
          {disabled ? (
            <Badge color="muted" className="ml-2">
              disabled
            </Badge>
          ) : null}
        </h2>
        <div className="mt-2">
          <DriftBadge state={drift?.state ?? null} />
        </div>
      </div>

      <div className="mb-3 flex items-center gap-2">
        <Button variant="secondary" size="sm" onClick={doToggle} disabled={anyBusy}>
          {disabled ? "Enable" : "Disable"}
        </Button>
        <div className="flex items-center gap-1">
          <select
            value={moveTo}
            onChange={(e) => setMoveTo(e.target.value as Scope)}
            disabled={anyBusy}
            className="rounded-md border border-border-default bg-surface-1 text-fg-default px-1 py-0.5 text-xs shadow-sm disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring-focus"
          >
            {ALL_SCOPES.map((s) => (
              <option key={s} value={s} disabled={s === scope}>
                {s}
                {s === scope ? " (current)" : ""}
              </option>
            ))}
          </select>
          <Button
            variant="secondary"
            size="sm"
            onClick={doMove}
            disabled={anyBusy || moveTo === scope}
          >
            Move to…
          </Button>
        </div>
        {onUninstall && (
          <Button
            variant="danger"
            size="sm"
            onClick={onUninstall}
            disabled={anyBusy}
            className="ml-auto"
          >
            {busy ? "Removing…" : "Uninstall"}
          </Button>
        )}
      </div>

      <div className="mb-3 rounded-md border border-border-default bg-surface-2 p-2 text-xs">
        <div className="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-1 font-mono">
          <span className="text-fg-muted">install</span>
          <span>{shortSha(drift?.installSha)}</span>
          <span className="text-fg-muted">disk</span>
          <span>{shortSha(drift?.currentSha)}</span>
          <span className="text-fg-muted">upstream</span>
          <span>{shortSha(drift?.upstreamSha)}</span>
        </div>
      </div>

      <div className="mb-3">
        <div className="mb-1 text-xs font-semibold uppercase text-fg-muted">
          Paths
        </div>
        <ul className="space-y-0.5 text-xs font-mono text-fg-default">
          {paths.map((p) => (
            <li key={p} className="truncate">
              {p}
            </li>
          ))}
        </ul>
      </div>

      {err && (
        <div className="mb-3 rounded-md border border-danger/40 bg-danger-subtle p-2 text-xs text-danger-fg">
          {err}
        </div>
      )}

      {body && body.frontmatter && (
        <div className="mb-3">
          <div className="mb-1 text-xs font-semibold uppercase text-fg-muted">
            Frontmatter
          </div>
          <pre className="whitespace-pre-wrap rounded-md border border-border-default bg-surface-2 p-2 font-mono text-xs text-fg-default">
            {body.frontmatter}
          </pre>
        </div>
      )}

      {body && (
        <div>
          <div className="mb-1 text-xs font-semibold uppercase text-fg-muted">
            Body
          </div>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap rounded-md border border-border-default bg-surface-2 p-2 font-mono text-xs text-fg-default">
            {body.body || "(empty)"}
          </pre>
        </div>
      )}
    </aside>
  );
}
