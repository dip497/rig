import { useState } from "react";
import type { AgentDto, Scope, UnitTypeId } from "../types";
import { installUnit } from "../lib/api";
import { Button, Input, Modal } from "../ui";

interface Props {
  agents: AgentDto[];
  scope: Scope;
  projectPath?: string;
  onClose: () => void;
  onInstalled: () => void;
}

const UNIT_TYPES: UnitTypeId[] = ["skill", "rule", "command", "subagent"];

export default function InstallModal({
  agents,
  scope,
  projectPath,
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
        projectPath: scope === "global" ? undefined : projectPath,
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
    <Modal title="Install unit" onClose={onClose} width="w-[520px]">
      <label className="mb-3 block">
        <span className="mb-1 block text-xs font-semibold uppercase text-fg-muted">
          Source
        </span>
        <Input
          type="text"
          value={source}
          onChange={(e) => setSource(e.target.value)}
          placeholder="./skill  or  ./skill.rig  or  local:./skill"
          className="w-full font-mono"
          autoFocus
        />
      </label>

      <label className="mb-3 block">
        <span className="mb-1 block text-xs font-semibold uppercase text-fg-muted">
          Type (optional)
        </span>
        <select
          value={asType}
          onChange={(e) => setAsType(e.target.value as UnitTypeId | "")}
          className="w-full rounded-md border border-border-default bg-surface-1 text-fg-default px-2 py-1 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring-focus"
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
        <div className="mb-1 text-xs font-semibold uppercase text-fg-muted">
          Agents
        </div>
        <div className="flex gap-3">
          {agents.map((a) => (
            <label
              key={a.id}
              className="flex items-center gap-1 text-sm text-fg-default"
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

      <div className="mb-3 text-xs text-fg-muted">
        Installing into <span className="font-mono">{scope}</span> scope.
      </div>

      {err && (
        <div className="mb-3 rounded-md border border-danger/40 bg-danger-subtle p-2 text-xs text-danger-fg">
          {err}
        </div>
      )}
      {result && (
        <pre className="mb-3 whitespace-pre-wrap rounded-md border border-success/40 bg-success-subtle p-2 font-mono text-xs text-success-fg">
          {result}
        </pre>
      )}

      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onClose}>
          Close
        </Button>
        <Button variant="primary" size="sm" onClick={submit} disabled={busy}>
          {busy ? "Installing…" : "Install"}
        </Button>
      </div>
    </Modal>
  );
}
