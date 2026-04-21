import type { Scope } from "../types";

export default function ScopePill({
  scope,
  onChange,
}: {
  scope: Scope;
  onChange: (s: Scope) => void;
}) {
  return (
    <div className="inline-flex rounded-md border border-slate-300 bg-white text-sm shadow-sm">
      {(["global", "project"] as Scope[]).map((s) => (
        <button
          key={s}
          onClick={() => onChange(s)}
          className={`px-3 py-1 first:rounded-l-md last:rounded-r-md ${
            scope === s
              ? "bg-indigo-600 text-white"
              : "text-slate-700 hover:bg-slate-50"
          }`}
        >
          {s}
        </button>
      ))}
    </div>
  );
}
