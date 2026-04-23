import type { InstalledUnitDto, Scope } from "../types";

const CURRENT_KEY = "rig-gui.current-project";
const RECENT_KEY = "rig-gui.recent-projects";
const MAX_RECENT = 5;

export function getCurrentProject(): string | null {
  try {
    return localStorage.getItem(CURRENT_KEY);
  } catch {
    return null;
  }
}

export function setCurrentProject(path: string | null) {
  try {
    if (path) {
      localStorage.setItem(CURRENT_KEY, path);
      const prev = getRecentProjects().filter((p) => p !== path);
      const next = [path, ...prev].slice(0, MAX_RECENT);
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    } else {
      localStorage.removeItem(CURRENT_KEY);
    }
  } catch {
    // ignore
  }
}

export function getRecentProjects(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

export interface OriginTaggedUnit extends InstalledUnitDto {
  origin: Scope;
  shadowed?: boolean;
}

/**
 * Merge units across scopes. `entries` are pairs of (scope, units).
 * Precedence (shown first, un-shadowed): project > local > global.
 * Shadow tag: a global row with same (agent,type,name) as a project/local row
 * stays in the list with `shadowed: true` so users learn precedence.
 */
export function mergeAcrossScopes(
  entries: { scope: Scope; units: InstalledUnitDto[] }[],
): OriginTaggedUnit[] {
  const rank: Record<Scope, number> = { project: 0, local: 1, global: 2 };
  const keyOf = (u: InstalledUnitDto) => `${u.agent}/${u.unitType}/${u.name}`;

  const winners = new Map<string, Scope>();
  for (const { scope, units } of entries) {
    for (const u of units) {
      const k = keyOf(u);
      const prev = winners.get(k);
      if (prev === undefined || rank[scope] < rank[prev]) {
        winners.set(k, scope);
      }
    }
  }

  const out: OriginTaggedUnit[] = [];
  for (const { scope, units } of entries) {
    for (const u of units) {
      const k = keyOf(u);
      const winner = winners.get(k)!;
      out.push({
        ...u,
        origin: scope,
        shadowed: scope === "global" && winner !== "global",
      });
    }
  }
  // Stable sort: winners first (by rank), then others.
  out.sort((a, b) => {
    if (a.shadowed !== b.shadowed) return a.shadowed ? 1 : -1;
    return rank[a.origin] - rank[b.origin];
  });
  return out;
}
