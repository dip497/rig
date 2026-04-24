import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type PropsWithChildren,
} from "react";

export type ThemeMode = "light" | "dark" | "system";
type Resolved = "light" | "dark";
const STORAGE_KEY = "rig-gui.theme";

interface ThemeCtx {
  mode: ThemeMode;
  resolved: Resolved;
  setMode: (m: ThemeMode) => void;
}
const Ctx = createContext<ThemeCtx | null>(null);

function mql(): MediaQueryList | null {
  try {
    return window.matchMedia("(prefers-color-scheme: dark)");
  } catch {
    return null;
  }
}

function resolve(mode: ThemeMode): Resolved {
  if (mode === "system") {
    return mql()?.matches ? "dark" : "light";
  }
  return mode;
}

function readStored(): ThemeMode {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* ignore */
  }
  return "system";
}

export function ThemeProvider({ children }: PropsWithChildren) {
  const [mode, setModeState] = useState<ThemeMode>(readStored);
  const [resolved, setResolved] = useState<Resolved>(() => resolve(readStored()));

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  useEffect(() => {
    setResolved(resolve(mode));
    if (mode !== "system") return;
    const m = mql();
    if (!m) return;
    const onChange = () => setResolved(m.matches ? "dark" : "light");
    m.addEventListener("change", onChange);
    return () => m.removeEventListener("change", onChange);
  }, [mode]);

  const setMode = useCallback((m: ThemeMode) => {
    try {
      localStorage.setItem(STORAGE_KEY, m);
    } catch {
      /* ignore */
    }
    setModeState(m);
  }, []);

  return <Ctx.Provider value={{ mode, resolved, setMode }}>{children}</Ctx.Provider>;
}

export function useTheme(): ThemeCtx {
  const v = useContext(Ctx);
  if (!v) throw new Error("useTheme outside ThemeProvider");
  return v;
}
