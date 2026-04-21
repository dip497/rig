import type { AgentDto } from "../types";

export default function Sidebar({
  agents,
  selected,
  onSelect,
}: {
  agents: AgentDto[];
  selected: string | null;
  onSelect: (id: string | null) => void;
}) {
  return (
    <aside className="w-56 border-r border-slate-200 bg-white p-3">
      <div className="mb-2 text-xs font-semibold uppercase text-slate-500">
        Agents
      </div>
      <ul className="space-y-1">
        <li>
          <button
            onClick={() => onSelect(null)}
            className={`w-full rounded px-2 py-1 text-left text-sm ${
              selected === null
                ? "bg-indigo-50 text-indigo-700"
                : "text-slate-700 hover:bg-slate-50"
            }`}
          >
            All agents
          </button>
        </li>
        {agents.map((a) => (
          <li key={a.id}>
            <button
              onClick={() => onSelect(a.id)}
              className={`w-full rounded px-2 py-1 text-left text-sm ${
                selected === a.id
                  ? "bg-indigo-50 text-indigo-700"
                  : "text-slate-700 hover:bg-slate-50"
              }`}
            >
              {a.id}
              <span className="ml-1 text-xs text-slate-400">
                ({a.capabilities.length})
              </span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
