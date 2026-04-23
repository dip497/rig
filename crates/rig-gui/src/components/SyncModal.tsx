import { useState } from "react";
import { syncScope } from "../lib/api";
import type { Scope, SyncResultDto } from "../types";

interface Props {
  scope: Scope;
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

export default function SyncModal({ scope, onClose, onDone }: Props) {
  const [mode, setMode] = useState<Mode>("keep");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [res, setRes] = useState<SyncResultDto | null>(null);

  const run = async () => {
    setBusy(true);
    setErr(null);
    setRes(null);
    try {
      const r = await syncScope(scope, mode);
      setRes(r);
      onDone();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-[560px] rounded-lg bg-white p-5 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">Sync bundles</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-700"
          >
            ✕
          </button>
        </div>

        <div className="mb-3 text-xs text-slate-500">
          Scope: <span className="font-mono">{scope}</span>. Reads
          <span className="font-mono"> .rig/rig.toml</span>, installs each
          bundle entry, rewrites lockfile.
        </div>

        <div className="mb-4">
          <div className="mb-1 text-xs font-semibold uppercase text-slate-500">
            On drift
          </div>
          <div className="space-y-1">
            {MODES.map((m) => (
              <label
                key={m.id}
                className="flex cursor-pointer items-start gap-2 rounded px-2 py-1 text-sm hover:bg-slate-50"
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
                  <div className="text-xs text-slate-500">{m.desc}</div>
                </div>
              </label>
            ))}
          </div>
          <div className="mt-2 text-xs text-slate-400">
            diff-per-file is CLI-only (interactive TTY).
          </div>
        </div>

        {err && (
          <div className="mb-3 rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
            {err}
          </div>
        )}

        {res && (
          <div className="mb-3 rounded border border-slate-200 bg-slate-50 p-2 text-xs">
            <div>
              Installed: <b>{res.installed.length}</b>, skipped:{" "}
              <b>{res.skipped.length}</b>, conflicts:{" "}
              <b>{res.conflicts.length}</b>
              {res.cancelled && (
                <span className="ml-2 rounded bg-yellow-100 px-2 text-yellow-800">
                  cancelled
                </span>
              )}
            </div>
            {res.skipped.length > 0 && (
              <pre className="mt-1 whitespace-pre-wrap font-mono text-[10px] text-slate-600">
                {res.skipped.join("\n")}
              </pre>
            )}
            {res.conflicts.length > 0 && (
              <pre className="mt-1 whitespace-pre-wrap font-mono text-[10px] text-red-700">
                {res.conflicts.join("\n")}
              </pre>
            )}
          </div>
        )}

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded border border-slate-300 bg-white px-3 py-1 text-sm hover:bg-slate-50"
          >
            Close
          </button>
          <button
            onClick={run}
            disabled={busy}
            className="rounded bg-indigo-600 px-3 py-1 text-sm text-white shadow-sm hover:bg-indigo-700 disabled:opacity-50"
          >
            {busy ? "Syncing…" : "Run Sync"}
          </button>
        </div>
      </div>
    </div>
  );
}
