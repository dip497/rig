import { useState } from "react";
import type { AgentDto, Scope, UnitTypeId } from "../types";
import { installUnit } from "../lib/api";

interface Props {
  agents: AgentDto[];
  scope: Scope;
  onClose: () => void;
  onInstalled: () => void;
}

const UNIT_TYPES: UnitTypeId[] = ["skill", "rule", "command", "subagent"];

export default function InstallModal({
  agents,
  scope,
  onClose,
  onInstalled,
}: Props) {
  const [source, setSource] = useState("");
  const [asType, setAsType] = useState<UnitTypeId | "">("");
  const [selectedAgents, setSelectedAgents] = useState<string[]>(
    agents.map((a) => a.id),
  );
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const toggleAgent = (id: string) => {
    setSelectedAgents((xs) =>
      xs.includes(id) ? xs.filter((x) => x !== id) : [...xs, id],
    );
  };

  const submit = async () => {
    if (!source.trim()) {
      setErr("source is required");
      return;
    }
    if (selectedAgents.length === 0) {
      setErr("pick at least one agent");
      return;
    }
    setBusy(true);
    setErr(null);
    setResult(null);
    try {
      const r = await installUnit({
        scope,
        source: source.trim(),
        agents: selectedAgents,
        asType: asType || undefined,
      });
      const lines: string[] = [];
      for (const u of r.installed) {
        lines.push(`+ ${u.agent} ${u.unitType}/${u.name}`);
      }
      for (const s of r.skipped) {
        lines.push(`~ ${s}`);
      }
      setResult(lines.join("\n") || "(no changes)");
      onInstalled();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-[520px] rounded-lg bg-white p-5 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">Install unit</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-700"
          >
            ✕
          </button>
        </div>

        <label className="mb-3 block">
          <span className="mb-1 block text-xs font-semibold uppercase text-slate-500">
            Source
          </span>
          <input
            type="text"
            value={source}
            onChange={(e) => setSource(e.target.value)}
            placeholder="./skill  or  ./skill.rig  or  local:./skill"
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm font-mono"
            autoFocus
          />
        </label>

        <label className="mb-3 block">
          <span className="mb-1 block text-xs font-semibold uppercase text-slate-500">
            Type (optional)
          </span>
          <select
            value={asType}
            onChange={(e) => setAsType(e.target.value as UnitTypeId | "")}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
          >
            <option value="">auto-detect</option>
            {UNIT_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </label>

        <div className="mb-3">
          <div className="mb-1 text-xs font-semibold uppercase text-slate-500">
            Agents
          </div>
          <div className="flex gap-3">
            {agents.map((a) => (
              <label
                key={a.id}
                className="flex items-center gap-1 text-sm"
              >
                <input
                  type="checkbox"
                  checked={selectedAgents.includes(a.id)}
                  onChange={() => toggleAgent(a.id)}
                />
                {a.id}
              </label>
            ))}
          </div>
        </div>

        <div className="mb-3 text-xs text-slate-500">
          Installing into <span className="font-mono">{scope}</span> scope.
        </div>

        {err && (
          <div className="mb-3 rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
            {err}
          </div>
        )}
        {result && (
          <pre className="mb-3 whitespace-pre-wrap rounded border border-green-200 bg-green-50 p-2 font-mono text-xs text-green-900">
            {result}
          </pre>
        )}

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded border border-slate-300 bg-white px-3 py-1 text-sm hover:bg-slate-50"
          >
            Close
          </button>
          <button
            onClick={submit}
            disabled={busy}
            className="rounded bg-indigo-600 px-3 py-1 text-sm text-white shadow-sm hover:bg-indigo-700 disabled:opacity-50"
          >
            {busy ? "Installing…" : "Install"}
          </button>
        </div>
      </div>
    </div>
  );
}
