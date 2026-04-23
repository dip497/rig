export type TypeFilter =
  | "all"
  | "skill"
  | "mcp"
  | "rule"
  | "command"
  | "subagent";

// Pills the user sees; "subagent" is in the full type union but shown as-is.
export const PILL_TYPES: TypeFilter[] = [
  "all",
  "skill",
  "mcp",
  "rule",
  "command",
  "subagent",
];

export default function TypeFilterPills({
  selected,
  counts,
  onChange,
}: {
  selected: TypeFilter;
  counts: Record<TypeFilter, number>;
  onChange: (t: TypeFilter) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1 border-b border-slate-200 bg-slate-50 px-3 py-2">
      {PILL_TYPES.map((t) => {
        const n = counts[t] ?? 0;
        const active = selected === t;
        const dim = n === 0 && !active;
        return (
          <button
            key={t}
            onClick={() => onChange(t)}
            className={`rounded-full px-3 py-0.5 text-xs capitalize transition ${
              active ? "bg-slate-900 text-white" : "hover:bg-slate-100 text-slate-700"
            } ${dim ? "opacity-50" : ""}`}
          >
            {t} ({n})
          </button>
        );
      })}
    </div>
  );
}
