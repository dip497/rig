import Pill from "./Pill";

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
    <div className="flex flex-wrap items-center gap-1">
      {PILL_TYPES.map((t) => {
        const n = counts[t] ?? 0;
        const active = selected === t;
        const dim = n === 0 && !active;
        return (
          <span key={t} className={dim ? "opacity-50" : ""}>
            <Pill active={active} onClick={() => onChange(t)}>
              <span className="capitalize">{t}</span>
              <span className="ml-1 text-xs opacity-70">({n})</span>
            </Pill>
          </span>
        );
      })}
    </div>
  );
}
