import type { ScopeSelection } from "../types";
import { Pill } from "../ui";

interface Props {
  scope: ScopeSelection;
  onChange: (s: ScopeSelection) => void;
  /** If no project is picked, "project", "local", and "all" are disabled. */
  hasProject?: boolean;
}

const OPTIONS: ScopeSelection[] = ["global", "project", "local", "all"];

export default function ScopePill({ scope, onChange, hasProject }: Props) {
  return (
    <div className="inline-flex items-center gap-1">
      {OPTIONS.map((s) => {
        const needsProject = s !== "global";
        const disabled = needsProject && !hasProject;
        return (
          <Pill
            key={s}
            active={scope === s}
            disabled={disabled}
            title={disabled ? "Open a project to use this scope" : undefined}
            onClick={() => onChange(s)}
          >
            {s}
          </Pill>
        );
      })}
    </div>
  );
}
