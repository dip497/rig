import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getRecentProjects } from "../lib/project";

interface Props {
  current: string | null;
  onPick: (path: string | null) => void;
}

export default function ProjectPicker({ current, onPick }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [recent, setRecent] = useState<string[]>(getRecentProjects());
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setRecent(getRecentProjects());
  }, [current, menuOpen]);

  useEffect(() => {
    const onDocClick = (e: MouseEvent) => {
      if (!ref.current) return;
      if (!ref.current.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, []);

  const pick = async () => {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "Open project",
      });
      if (typeof result === "string") onPick(result);
    } catch (e) {
      console.error("project picker failed:", e);
    }
  };

  return (
    <div className="flex items-center gap-2" ref={ref}>
      <button
        onClick={pick}
        className="rounded border border-slate-300 bg-white px-2 py-1 text-sm shadow-sm hover:bg-slate-50"
      >
        Open project…
      </button>

      {current && (
        <span className="inline-flex items-center gap-1 rounded bg-slate-100 px-2 py-0.5 text-xs text-slate-700">
          <span className="max-w-[240px] truncate font-mono" title={current}>
            {current}
          </span>
          <button
            onClick={() => onPick(null)}
            className="ml-1 text-slate-500 hover:text-slate-900"
            title="Clear project"
          >
            ×
          </button>
        </span>
      )}

      <div className="relative">
        <button
          onClick={() => setMenuOpen((x) => !x)}
          className="rounded border border-slate-300 bg-white px-2 py-1 text-xs text-slate-700 shadow-sm hover:bg-slate-50"
        >
          Recent ▾
        </button>
        {menuOpen && (
          <div className="absolute right-0 z-50 mt-1 w-80 rounded border border-slate-200 bg-white p-1 text-xs shadow-lg">
            {recent.length === 0 ? (
              <div className="px-2 py-2 text-slate-500">No recent projects</div>
            ) : (
              recent.map((p) => (
                <button
                  key={p}
                  onClick={() => {
                    setMenuOpen(false);
                    onPick(p);
                  }}
                  className={`block w-full truncate rounded px-2 py-1 text-left font-mono hover:bg-slate-100 ${
                    p === current ? "text-indigo-700" : "text-slate-700"
                  }`}
                  title={p}
                >
                  {p}
                </button>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}
