import type { ScopeSelection } from "../types";

interface Props {
  scope: ScopeSelection;
  onChange: (s: ScopeSelection) => void;
  /** If no project is picked, "project", "local", and "all" are disabled. */
  hasProject?: boolean;
}

const OPTIONS: ScopeSelection[] = ["global", "project", "local", "all"];

export default function ScopePill({ scope, onChange, hasProject }: Props) {
  return (
    <div className="inline-flex rounded-md border border-slate-300 bg-white text-sm shadow-sm">
      {OPTIONS.map((s) => {
        const needsProject = s !== "global";
        const disabled = needsProject && !hasProject;
        return (
          <button
            key={s}
            onClick={() => !disabled && onChange(s)}
            disabled={disabled}
            title={disabled ? "Open a project to use this scope" : undefined}
            className={`px-3 py-1 first:rounded-l-md last:rounded-r-md ${
              scope === s
                ? "bg-indigo-600 text-white"
                : "text-slate-700 hover:bg-slate-50"
            } ${disabled ? "cursor-not-allowed opacity-40 hover:bg-white" : ""}`}
          >
            {s}
          </button>
        );
      })}
    </div>
  );
}
