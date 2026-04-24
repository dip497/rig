import { useState } from "react";
import { syncScope } from "../lib/api";
import type { Scope, SyncResultDto } from "../types";
import { Badge, Button, Modal } from "../ui";

interface Props {
  scope: Scope;
  projectPath?: string;
  onClose: () => void;
  onDone: () => void;
}

type Mode = "keep" | "overwrite" | "snapshot-then-overwrite" | "cancel";

const MODES: { id: Mode; label: string; desc: string }[] = [
  { id: "keep", label: "keep", desc: "Skip local drift; leave files alone." },
  { id: "overwrite", label: "overwrite", desc: "Clobber local changes with upstream." },
  {
    id: "snapshot-then-overwrite",
    label: "snapshot + overwrite",
    desc: "Rename existing files to .rig-backup-<ts>, then write.",
  },
  { id: "cancel", label: "cancel", desc: "Abort on first drift." },
];

export default function SyncModal({ scope, projectPath, onClose, onDone }: Props) {
  const [mode, setMode] = useState<Mode>("keep");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [res, setRes] = useState<SyncResultDto | null>(null);

  const run = async () => {
    setBusy(true);
    setErr(null);
    setRes(null);
    try {
      const r = await syncScope(
        scope,
        mode,
        scope === "global" ? undefined : projectPath,
      );
      setRes(r);
      onDone();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Sync bundles" onClose={onClose} width="w-[560px]">
      <div className="mb-3 text-xs text-fg-muted">
        Scope: <span className="font-mono">{scope}</span>. Reads
        <span className="font-mono"> .rig/rig.toml</span>, installs each
        bundle entry, rewrites lockfile.
      </div>

      <div className="mb-4">
        <div className="mb-1 text-xs font-semibold uppercase text-fg-muted">
          On drift
        </div>
        <div className="space-y-1">
          {MODES.map((m) => (
            <label
              key={m.id}
              className="flex cursor-pointer items-start gap-2 rounded-md px-2 py-1 text-sm text-fg-default hover:bg-surface-2"
            >
              <input
                type="radio"
                className="mt-0.5"
                name="mode"
                checked={mode === m.id}
                onChange={() => setMode(m.id)}
              />
              <div>
                <div className="font-mono">{m.label}</div>
                <div className="text-xs text-fg-muted">{m.desc}</div>
              </div>
            </label>
          ))}
        </div>
        <div className="mt-2 text-xs text-fg-subtle">
          diff-per-file is CLI-only (interactive TTY).
        </div>
      </div>

      {err && (
        <div className="mb-3 rounded-md border border-danger/40 bg-danger-subtle p-2 text-xs text-danger-fg">
          {err}
        </div>
      )}

      {res && (
        <div className="mb-3 rounded-md border border-border-default bg-surface-2 p-2 text-xs text-fg-default">
          <div>
            Installed: <b>{res.installed.length}</b>, skipped:{" "}
            <b>{res.skipped.length}</b>, conflicts:{" "}
            <b>{res.conflicts.length}</b>
            {res.cancelled && (
              <Badge color="warning" className="ml-2">
                cancelled
              </Badge>
            )}
          </div>
          {res.skipped.length > 0 && (
            <pre className="mt-1 whitespace-pre-wrap font-mono text-[10px] text-fg-muted">
              {res.skipped.join("\n")}
            </pre>
          )}
          {res.conflicts.length > 0 && (
            <pre className="mt-1 whitespace-pre-wrap font-mono text-[10px] text-danger-fg">
              {res.conflicts.join("\n")}
            </pre>
          )}
        </div>
      )}

      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onClose}>
          Close
        </Button>
        <Button variant="primary" size="sm" onClick={run} disabled={busy}>
          {busy ? "Syncing…" : "Run Sync"}
        </Button>
      </div>
    </Modal>
  );
}
