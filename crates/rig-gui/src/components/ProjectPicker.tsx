import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getRecentProjects } from "../lib/project";
import { Badge, Button } from "../ui";

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
      <Button variant="secondary" size="sm" onClick={pick}>
        Open project…
      </Button>

      {current && (
        <Badge color="muted" className="gap-1">
          <span className="max-w-[240px] truncate font-mono" title={current}>
            {current}
          </span>
          <button
            onClick={() => onPick(null)}
            className="ml-1 text-fg-subtle hover:text-fg-default"
            title="Clear project"
          >
            ×
          </button>
        </Badge>
      )}

      <div className="relative">
        <Button variant="secondary" size="sm" onClick={() => setMenuOpen((x) => !x)}>
          Recent ▾
        </Button>
        {menuOpen && (
          <div className="absolute right-0 z-50 mt-1 w-80 rounded-md border border-border-default bg-surface-1 p-1 text-xs shadow-pop">
            {recent.length === 0 ? (
              <div className="px-2 py-2 text-fg-muted">No recent projects</div>
            ) : (
              recent.map((p) => (
                <button
                  key={p}
                  onClick={() => {
                    setMenuOpen(false);
                    onPick(p);
                  }}
                  className={`block w-full truncate rounded-md px-2 py-1 text-left font-mono hover:bg-surface-2 ${
                    p === current ? "text-accent-primary" : "text-fg-default"
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
