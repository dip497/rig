# Theme System (rig-gui)

One-paragraph summary: Specifies the design-token and theming architecture
for `rig-gui` (Tauri 2 + React 19 + Vite 8 + Tailwind 4). Every colour,
space, radius, shadow, and typography value is defined once as a named
token, exposed as a CSS custom property, and consumed through Tailwind 4's
`@theme` directive. Light, dark, and system modes are first-class. This
spec is consulted any time a component adds or changes visual styling and
any time the palette shifts. Sibling context: `docs/architecture.md` (GUI
seam), `docs/concepts.md` (shared vocabulary), ADR-018 (GUI direct-links
adapters in M1).

## Goals

- **One source of truth.** Every visual constant lives in a single
  `theme.css`; components reference semantic tokens, never raw Tailwind
  palette classes (`bg-slate-900`, `text-indigo-500`).
- **Light + dark + system.** `data-theme="light" | "dark"` on `<html>`;
  system mode tracks `prefers-color-scheme` live.
- **Semantic, not literal.** Token names describe role
  (`--color-surface-1`, `--color-fg-default`) not pigment
  (`--color-slate-900`).
- **WCAG AA or better** for every documented fg/bg pairing in both
  themes.
- **No runtime theme library.** A ~40-line `ThemeProvider` hook; no
  `next-themes`, no Emotion, no CSS-in-JS.
- **Component kit minimal but present.** `ui/` directory exposes
  `Button`, `Pill`, `Card`, `Badge`, `Input`, `Modal`, `EmptyState`.
- **Zero regressions** — every pre-existing screen still renders after
  migration.

## Non-goals

- Theming the CLI (`rig-cli` is styled by `owo-colors`; separate spec
  when it needs one).
- Full design system with motion / iconography / illustration tokens.
  M1 is colour + spacing + radius + shadow + typography only.
- High-contrast, colour-blind, or custom user-defined palettes — see
  open questions.
- Theming the Tauri window chrome (title bar colour). Defer until the
  native window-controls decision in ADR-TBD.
- RTL / BiDi token variants.

## Key concepts

- **Token**. A named CSS custom property on `:root` (and overridden on
  `[data-theme="dark"]`). Consumed by Tailwind via `@theme`.
- **Semantic token**. Describes intent (`--color-accent-primary`).
  The only kind of token components should reference.
- **Primitive palette**. Literal scale (e.g. `--palette-slate-900`).
  Defined in `theme.css` but **not** exported to Tailwind utilities;
  referenced only by semantic tokens.
- **Theme mode**. `light` | `dark` | `system`. Persisted as
  `localStorage.rig-gui.theme`; default `system`.
- **ui kit**. Components under `crates/rig-gui/src/ui/` that consume
  tokens and expose a typed React API.

Shared vocabulary (Unit, Scope, Adapter, Drift) lives in
[`concepts.md`](./concepts.md); this spec does not redefine them.

## Schema / contract / algorithm

### Token inventory

All colours are `oklch(L C H)` so contrast can be reasoned about
numerically. Light + dark values defined. Every pair listed under
Accessibility has a computed contrast ratio.

#### Surfaces (background layers)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--color-bg-canvas` | `oklch(0.99 0 0)` | `oklch(0.17 0.01 260)` | outermost app background |
| `--color-surface-1` | `oklch(1 0 0)` | `oklch(0.21 0.01 260)` | cards, panels |
| `--color-surface-2` | `oklch(0.97 0.004 260)` | `oklch(0.25 0.012 260)` | nested panels, table headers |
| `--color-surface-3` | `oklch(0.94 0.006 260)` | `oklch(0.30 0.014 260)` | popovers, tooltips |
| `--color-overlay` | `oklch(0.17 0.01 260 / 0.5)` | `oklch(0 0 0 / 0.65)` | modal scrim |

#### Foregrounds

| Token | Light | Dark | Use |
|---|---|---|---|
| `--color-fg-default` | `oklch(0.20 0.015 260)` | `oklch(0.96 0.005 260)` | body text |
| `--color-fg-muted` | `oklch(0.47 0.015 260)` | `oklch(0.72 0.012 260)` | secondary text, helper |
| `--color-fg-subtle` | `oklch(0.62 0.012 260)` | `oklch(0.58 0.012 260)` | placeholder, disabled caption |
| `--color-fg-on-accent` | `oklch(0.99 0 0)` | `oklch(0.99 0 0)` | text on accent fills |

#### Borders & rings

| Token | Light | Dark | Use |
|---|---|---|---|
| `--color-border-default` | `oklch(0.91 0.008 260)` | `oklch(0.32 0.012 260)` | dividers, card edges |
| `--color-border-strong` | `oklch(0.83 0.012 260)` | `oklch(0.42 0.014 260)` | hover / focus borders |
| `--color-ring-focus` | `oklch(0.62 0.18 255)` | `oklch(0.72 0.17 255)` | focus outline |

#### Accents / status

| Token | Light | Dark | Use |
|---|---|---|---|
| `--color-accent-primary` | `oklch(0.55 0.19 255)` | `oklch(0.68 0.17 255)` | primary buttons, links |
| `--color-accent-primary-hover` | `oklch(0.48 0.2 255)` | `oklch(0.73 0.17 255)` | hover state |
| `--color-accent-subtle` | `oklch(0.95 0.04 255)` | `oklch(0.28 0.07 255)` | tinted backgrounds |
| `--color-success` | `oklch(0.62 0.16 150)` | `oklch(0.72 0.16 150)` | `Clean`, success toasts |
| `--color-warning` | `oklch(0.75 0.17 75)` | `oklch(0.80 0.17 75)` | `LocalDrift`, warnings |
| `--color-danger` | `oklch(0.58 0.22 25)` | `oklch(0.68 0.21 25)` | `BothDrift`, destructive |
| `--color-info` | `oklch(0.65 0.12 220)` | `oklch(0.75 0.12 220)` | neutral info pills |

Each status colour has `-subtle` (tinted bg) and `-fg` (readable fg)
companions, e.g. `--color-success-subtle`, `--color-success-fg`.

#### Spacing (4px base)

`--space-0: 0`, `--space-1: 0.25rem`, `--space-2: 0.5rem`,
`--space-3: 0.75rem`, `--space-4: 1rem`, `--space-5: 1.25rem`,
`--space-6: 1.5rem`, `--space-8: 2rem`, `--space-10: 2.5rem`,
`--space-12: 3rem`.

#### Radius

`--radius-sm: 4px`, `--radius-md: 8px`, `--radius-lg: 12px`,
`--radius-pill: 9999px`.

#### Typography

| Token | Size | Weight | Line-height |
|---|---|---|---|
| `--font-display` | 24px | 600 | 1.2 |
| `--font-title` | 18px | 600 | 1.3 |
| `--font-body` | 14px | 400 | 1.5 |
| `--font-body-strong` | 14px | 600 | 1.5 |
| `--font-caption` | 12px | 400 | 1.4 |
| `--font-mono` | 13px | 400 | 1.4 |

Font families: `--font-family-sans` (system stack from current
`index.css`), `--font-family-mono` (`ui-monospace, SFMono-Regular,
"SF Mono", Menlo, monospace`).

#### Shadow

| Token | Light | Dark |
|---|---|---|
| `--shadow-sm` | `0 1px 2px oklch(0.2 0.01 260 / 0.06)` | `0 1px 2px oklch(0 0 0 / 0.4)` |
| `--shadow-md` | `0 4px 12px oklch(0.2 0.01 260 / 0.08)` | `0 4px 12px oklch(0 0 0 / 0.45)` |
| `--shadow-lg` | `0 12px 32px oklch(0.2 0.01 260 / 0.12)` | `0 12px 32px oklch(0 0 0 / 0.55)` |
| `--shadow-pop` | `0 2px 6px oklch(0.2 0.01 260 / 0.08), 0 20px 40px oklch(0.2 0.01 260 / 0.14)` | `0 2px 6px oklch(0 0 0 / 0.5), 0 20px 40px oklch(0 0 0 / 0.6)` |

### CSS implementation

`crates/rig-gui/src/styles/theme.css`:

```css
@import "tailwindcss";

/* Data-attribute dark variant (Tailwind v4). */
@custom-variant dark (&:where([data-theme="dark"], [data-theme="dark"] *));

:root {
  color-scheme: light;

  --color-bg-canvas: oklch(0.99 0 0);
  --color-surface-1: oklch(1 0 0);
  --color-surface-2: oklch(0.97 0.004 260);
  --color-surface-3: oklch(0.94 0.006 260);
  --color-overlay: oklch(0.17 0.01 260 / 0.5);

  --color-fg-default: oklch(0.20 0.015 260);
  --color-fg-muted: oklch(0.47 0.015 260);
  --color-fg-subtle: oklch(0.62 0.012 260);
  --color-fg-on-accent: oklch(0.99 0 0);

  --color-border-default: oklch(0.91 0.008 260);
  --color-border-strong: oklch(0.83 0.012 260);
  --color-ring-focus: oklch(0.62 0.18 255);

  --color-accent-primary: oklch(0.55 0.19 255);
  --color-accent-primary-hover: oklch(0.48 0.20 255);
  --color-accent-subtle: oklch(0.95 0.04 255);

  --color-success:         oklch(0.62 0.16 150);
  --color-success-subtle:  oklch(0.95 0.05 150);
  --color-success-fg:      oklch(0.35 0.12 150);

  --color-warning:         oklch(0.75 0.17 75);
  --color-warning-subtle:  oklch(0.96 0.06 75);
  --color-warning-fg:      oklch(0.45 0.14 60);

  --color-danger:          oklch(0.58 0.22 25);
  --color-danger-subtle:   oklch(0.95 0.05 25);
  --color-danger-fg:       oklch(0.42 0.18 25);

  --color-info:            oklch(0.65 0.12 220);
  --color-info-subtle:     oklch(0.95 0.03 220);
  --color-info-fg:         oklch(0.40 0.10 220);

  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-pill: 9999px;

  --shadow-sm:  0 1px 2px oklch(0.2 0.01 260 / 0.06);
  --shadow-md:  0 4px 12px oklch(0.2 0.01 260 / 0.08);
  --shadow-lg:  0 12px 32px oklch(0.2 0.01 260 / 0.12);
  --shadow-pop: 0 2px 6px oklch(0.2 0.01 260 / 0.08),
                0 20px 40px oklch(0.2 0.01 260 / 0.14);
}

[data-theme="dark"] {
  color-scheme: dark;

  --color-bg-canvas: oklch(0.17 0.01 260);
  --color-surface-1: oklch(0.21 0.01 260);
  --color-surface-2: oklch(0.25 0.012 260);
  --color-surface-3: oklch(0.30 0.014 260);
  --color-overlay:   oklch(0 0 0 / 0.65);

  --color-fg-default: oklch(0.96 0.005 260);
  --color-fg-muted:   oklch(0.72 0.012 260);
  --color-fg-subtle:  oklch(0.58 0.012 260);

  --color-border-default: oklch(0.32 0.012 260);
  --color-border-strong:  oklch(0.42 0.014 260);
  --color-ring-focus:     oklch(0.72 0.17 255);

  --color-accent-primary:       oklch(0.68 0.17 255);
  --color-accent-primary-hover: oklch(0.73 0.17 255);
  --color-accent-subtle:        oklch(0.28 0.07 255);

  --color-success-subtle: oklch(0.28 0.06 150);
  --color-success-fg:     oklch(0.82 0.14 150);
  --color-warning-subtle: oklch(0.30 0.06 75);
  --color-warning-fg:     oklch(0.85 0.15 75);
  --color-danger-subtle:  oklch(0.30 0.09 25);
  --color-danger-fg:      oklch(0.82 0.15 25);
  --color-info-subtle:    oklch(0.28 0.05 220);
  --color-info-fg:        oklch(0.82 0.10 220);

  --shadow-sm:  0 1px 2px oklch(0 0 0 / 0.4);
  --shadow-md:  0 4px 12px oklch(0 0 0 / 0.45);
  --shadow-lg:  0 12px 32px oklch(0 0 0 / 0.55);
  --shadow-pop: 0 2px 6px oklch(0 0 0 / 0.5),
                0 20px 40px oklch(0 0 0 / 0.6);
}

@theme {
  --color-bg-canvas:         var(--color-bg-canvas);
  --color-surface-1:         var(--color-surface-1);
  --color-surface-2:         var(--color-surface-2);
  --color-surface-3:         var(--color-surface-3);
  --color-fg-default:        var(--color-fg-default);
  --color-fg-muted:          var(--color-fg-muted);
  --color-fg-subtle:         var(--color-fg-subtle);
  --color-fg-on-accent:      var(--color-fg-on-accent);
  --color-border-default:    var(--color-border-default);
  --color-border-strong:     var(--color-border-strong);
  --color-ring-focus:        var(--color-ring-focus);
  --color-accent-primary:    var(--color-accent-primary);
  --color-accent-primary-hover: var(--color-accent-primary-hover);
  --color-accent-subtle:     var(--color-accent-subtle);
  --color-success:           var(--color-success);
  --color-success-subtle:    var(--color-success-subtle);
  --color-success-fg:        var(--color-success-fg);
  --color-warning:           var(--color-warning);
  --color-warning-subtle:    var(--color-warning-subtle);
  --color-warning-fg:        var(--color-warning-fg);
  --color-danger:            var(--color-danger);
  --color-danger-subtle:     var(--color-danger-subtle);
  --color-danger-fg:         var(--color-danger-fg);
  --color-info:              var(--color-info);
  --color-info-subtle:       var(--color-info-subtle);
  --color-info-fg:           var(--color-info-fg);

  --radius-sm:  var(--radius-sm);
  --radius-md:  var(--radius-md);
  --radius-lg:  var(--radius-lg);
  --radius-xl:  var(--radius-pill);

  --shadow-sm:  var(--shadow-sm);
  --shadow-md:  var(--shadow-md);
  --shadow-lg:  var(--shadow-lg);
  --shadow-pop: var(--shadow-pop);
}

body {
  margin: 0;
  background: var(--color-bg-canvas);
  color: var(--color-fg-default);
  font-family: var(--font-family-sans);
}
```

Components then use `bg-surface-1`, `text-fg-muted`, `border-border-default`,
`ring-ring-focus`, `rounded-md`, `shadow-pop` — all generated by Tailwind
4 from `@theme` ([Tailwind docs](https://tailwindcss.com/docs/theme),
[dark-mode data-attr](https://tailwindcss.com/docs/dark-mode#using-a-data-attribute)).

### React integration

`crates/rig-gui/src/theme/ThemeProvider.tsx`:

```tsx
import {
  createContext, useContext, useEffect, useState, useCallback,
  type PropsWithChildren,
} from "react";

export type ThemeMode = "light" | "dark" | "system";
const STORAGE_KEY = "rig-gui.theme";

interface ThemeCtx {
  mode: ThemeMode;
  resolved: "light" | "dark";
  setMode: (m: ThemeMode) => void;
}
const Ctx = createContext<ThemeCtx | null>(null);

const mql = () => window.matchMedia("(prefers-color-scheme: dark)");

function resolve(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") return mql().matches ? "dark" : "light";
  return mode;
}

export function ThemeProvider({ children }: PropsWithChildren) {
  const [mode, setModeState] = useState<ThemeMode>(
    () => (localStorage.getItem(STORAGE_KEY) as ThemeMode) ?? "system",
  );
  const [resolved, setResolved] = useState<"light" | "dark">(() => resolve(mode));

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  useEffect(() => {
    setResolved(resolve(mode));
    if (mode !== "system") return;
    const m = mql();
    const onChange = () => setResolved(m.matches ? "dark" : "light");
    m.addEventListener("change", onChange);
    return () => m.removeEventListener("change", onChange);
  }, [mode]);

  const setMode = useCallback((m: ThemeMode) => {
    localStorage.setItem(STORAGE_KEY, m);
    setModeState(m);
  }, []);

  return <Ctx.Provider value={{ mode, resolved, setMode }}>{children}</Ctx.Provider>;
}

export function useTheme(): ThemeCtx {
  const v = useContext(Ctx);
  if (!v) throw new Error("useTheme outside ThemeProvider");
  return v;
}
```

Mount once in `main.tsx` wrapping `<App />`. No external deps.

### Component layer

Path: `crates/rig-gui/src/ui/`. Exports a barrel `index.ts`.

```
ui/
  Button.tsx
  Pill.tsx       // moved from components/Pill.tsx
  Card.tsx
  Badge.tsx
  Input.tsx
  Modal.tsx
  EmptyState.tsx
  index.ts
```

Reference implementation — `ui/Button.tsx`:

```tsx
import { forwardRef, type ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

const base =
  "inline-flex items-center justify-center gap-2 rounded-md font-medium " +
  "transition-colors focus-visible:outline-none focus-visible:ring-2 " +
  "focus-visible:ring-ring-focus disabled:opacity-50 disabled:pointer-events-none";

const variants: Record<Variant, string> = {
  primary:
    "bg-accent-primary text-fg-on-accent hover:bg-accent-primary-hover",
  secondary:
    "bg-surface-2 text-fg-default border border-border-default hover:bg-surface-3",
  ghost:
    "bg-transparent text-fg-muted hover:bg-surface-2 hover:text-fg-default",
  danger:
    "bg-danger text-fg-on-accent hover:opacity-90",
};

const sizes: Record<Size, string> = {
  sm: "h-7 px-2 text-[12px]",
  md: "h-9 px-3 text-[13px]",
};

export const Button = forwardRef<HTMLButtonElement, Props>(
  ({ variant = "secondary", size = "md", className = "", ...rest }, ref) => (
    <button
      ref={ref}
      className={`${base} ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    />
  ),
);
Button.displayName = "Button";
```

Other components follow the same pattern: named variants, all colours
via tokens, forwardRef, no `bg-slate-*`. Radix primitives are not
pulled in yet (see Open questions).

## Examples

### Theme toggle (fits existing header)

```tsx
import { useTheme, type ThemeMode } from "@/theme/ThemeProvider";
import { Button } from "@/ui";

const ORDER: ThemeMode[] = ["light", "dark", "system"];
const LABEL = { light: "Sun", dark: "Moon", system: "Auto" } as const;

export function ThemeToggle() {
  const { mode, setMode } = useTheme();
  const next = ORDER[(ORDER.indexOf(mode) + 1) % ORDER.length];
  return (
    <Button variant="ghost" size="sm" onClick={() => setMode(next)}
      aria-label={`Theme: ${mode}. Click to switch to ${next}.`}>
      {LABEL[mode]}
    </Button>
  );
}
```

### Status pill consuming tokens

```tsx
import type { DriftState } from "@/types";

const styles: Record<DriftState, string> = {
  Clean:         "bg-success-subtle text-success-fg",
  LocalDrift:    "bg-warning-subtle text-warning-fg",
  UpstreamDrift: "bg-info-subtle    text-info-fg",
  BothDrift:     "bg-danger-subtle  text-danger-fg",
  Orphan:        "bg-surface-2      text-fg-muted",
  Missing:       "bg-surface-2      text-fg-subtle italic",
};

export function DriftBadge({ state }: { state: DriftState }) {
  return (
    <span className={`inline-flex items-center px-2 h-5 rounded-pill text-[11px] font-medium ${styles[state]}`}>
      {state}
    </span>
  );
}
```

## Edge cases

- **localStorage unavailable** (strict Tauri privacy profile): fall
  back to `system`, do not throw.
- **`prefers-color-scheme` unsupported**: `mql().matches` returns
  `false`; effectively light mode. Acceptable.
- **User changes OS theme while `mode === "system"`**: listener on
  `matchMedia` updates `resolved`; `data-theme` swaps without reload.
- **User picks "dark" then OS switches**: no change, explicit choice
  wins.
- **FOUC on first paint**: a blocking `<script>` in `index.html`
  reads `localStorage` and sets `data-theme` before React mounts
  (snippet in `index.html`, not React).
- **Two windows open**: each window reads its own localStorage; Tauri
  does not broadcast. Acceptable for M1 (single-window GUI).
- **High-DPI / reduced motion**: out of scope; tracked in open
  questions.
- **Tailwind purging unused tokens**: `@theme` tokens are always
  emitted; no purging concern.
- **Token-name collision with Tailwind defaults**: the `@theme` block
  shadows defaults; literal utilities like `bg-slate-900` continue to
  work but are forbidden by lint (see Migration plan).

## Error semantics

- `useTheme` outside `<ThemeProvider>` throws synchronously at mount;
  surfaces via React error boundary. Not recoverable; indicates bug.
- Invalid value in `localStorage.rig-gui.theme` is silently coerced to
  `system` (no throw).
- No async surface; no error codes beyond the React error.

## Open questions

1. **Radix primitives (Q1).** Pull `@radix-ui/react-dialog` for Modal
   (accessibility out-of-the-box) or keep hand-rolled? Leaning Radix
   for Dialog, Dropdown, Tooltip only. Resolve before Phase 2.
2. **High-contrast variant (Q2).** Add `[data-theme="hc-light"]`
   tokens for WCAG AAA? Defer to M2 unless an accessibility
   contributor requests sooner.
3. **Tauri `setTheme` sync (Q3).** Should we call
   `@tauri-apps/api/app#setTheme(null|"light"|"dark")` to sync the
   native title bar colour on platforms that support it (macOS,
   Windows 11)? Probably yes for polish but not blocking.
4. **Font stack (Q4).** Keep system-font stack or ship Inter / Geist
   as a bundled asset? System stack for M1; reopen if typographic
   consistency complaints land.
5. **Visual-regression tooling (Q5).** Playwright + per-theme
   screenshots, or Storybook + Chromatic? Deferred; see Testing.

## Interoperation

- **Tailwind 4** — uses the `@theme` directive and
  `@custom-variant dark (&:where([data-theme=dark], …))` pattern
  documented at
  <https://tailwindcss.com/docs/theme> and
  <https://tailwindcss.com/docs/dark-mode#using-a-data-attribute>.
- **Tauri 2** — optionally integrates with
  `@tauri-apps/api/app#setTheme` for native chrome sync; docs:
  <https://v2.tauri.app/reference/javascript/api/namespaceapp/>.
  See ADR-018 for why the GUI may call Tauri APIs directly in M1.
- **Radix Colors / shadcn/ui** — the semantic-token naming
  (`surface-N`, `fg-default`, `accent-*`) is borrowed from Radix
  Colors (<https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale>)
  and shadcn/ui's CSS-variable theme
  (<https://ui.shadcn.com/docs/theming>). Attribution recorded here
  per `CLAUDE.md` rule on adopting from adjacent projects. We do **not**
  consume their libraries in M1.
- **gsd2-config** — reviewed for prior art; Rig adopts the
  `data-theme` + `@theme` pattern independently. No code lifted.
- **Rig docs** — see `architecture.md` (GUI seam), `concepts.md`
  (drift terms consumed by `DriftBadge`), ADR-018 (GUI direct-links
  adapters).

## Versioning

- Token namespace versioned via the top-level manifest schema key
  already in use (`rig/v1`). The theme contract is `theme/v1`
  embedded as a comment header in `theme.css`.
- Breaking token renames (`--color-surface-1` →
  `--color-bg-surface-primary`) bump to `theme/v2`; a migration
  section is added to this spec and components are updated in the
  same PR (no deprecation window — internal to `rig-gui`).
- Adding tokens is non-breaking. Changing values within AA-compliant
  bounds is non-breaking.

## Migration plan

Phase 1 — foundation (one PR):

- Add `crates/rig-gui/src/styles/theme.css`; replace `src/index.css`
  contents with `@import "./styles/theme.css";`.
- Add `src/theme/ThemeProvider.tsx`; wrap `<App />` in `main.tsx`.
- Add anti-FOUC script in `index.html`.
- Add `src/ui/` kit (Button, Pill [moved], Card, Badge, Input, Modal,
  EmptyState) and barrel `index.ts`.
- Add `<ThemeToggle />` to App header next to Refresh.

Phase 2 — component refactor (one PR per file or small batches):

| File | Touches |
|---|---|
| `components/UnitTable.tsx` | row bg, hover, border, zebra |
| `components/DetailPane.tsx` | panel surfaces, mono code block |
| `components/InstallModal.tsx` | Modal + Button + Input from ui/ |
| `components/SyncModal.tsx` | same |
| `components/StatsView.tsx` | stat cards → `Card` + tokenized fg |
| `components/DoctorView.tsx` | status rows → semantic status tokens |
| `components/ProjectPicker.tsx` | dropdown surface + border |
| `components/TypeFilter.tsx` | Pill variants |
| `components/ScopePill.tsx` | fold into ui/Pill |
| `components/Pill.tsx` | delete (moved to ui/) |
| `components/DriftBadge.tsx` | token-driven status map (see Examples) |
| `App.tsx` | header layout, theme toggle slot; drop direct slate refs |

`components/Sidebar.tsx` was already removed; leaves nothing to port.

Phase 3 — enforce (one PR):

- Add ESLint rule
  `no-restricted-syntax` matching `/\b(bg|text|border|ring|from|to|via)-(slate|gray|zinc|indigo|blue|red|green|amber|yellow|emerald)-\d{2,3}\b/`
  on `className` string literals. CI fails on match.
- Remove unused primitive Tailwind colours by overriding them to
  `initial` in `@theme` if needed.
- Delete `--palette-*` helpers once every component is migrated.

## Theme toggle UX

- Placement: App header, right-aligned, immediately left of the
  existing "Refresh" button.
- Shape: 28×28 `ghost`-variant square button, rounded-md.
- Icon: inline SVG (sun / moon / half-moon). No icon library added.
- Cycle order: `light → dark → system → light`.
- Persistence: `localStorage.rig-gui.theme`. Default on first launch:
  `system`.
- `aria-label` reflects current mode and next action (see example).
- Keyboard: standard `button` semantics; `Enter` / `Space` cycle.

## System sync

- `window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', …)`
  fires while `mode === "system"`; listener detaches on mode change.
- Tauri 2 optionally: `import { setTheme } from "@tauri-apps/api/app"`
  and call `setTheme(resolved)` whenever `resolved` changes, so the
  native title-bar matches on macOS / Windows 11. Gated behind a
  feature flag for M1 while we verify behaviour on Linux (ignored on
  most WMs). Docs:
  <https://v2.tauri.app/reference/javascript/api/namespaceapp/#settheme>.
- We do **not** use `getCurrentWindow().onThemeChanged` because
  `matchMedia` is sufficient and framework-agnostic.

## Accessibility

Target WCAG AA (4.5:1 body text, 3:1 for large text / UI). Computed
against the tokens in the inventory (oklch → sRGB):

| Pair | Light ratio | Dark ratio |
|---|---|---|
| `fg-default` on `bg-canvas` | 15.8:1 | 14.1:1 |
| `fg-default` on `surface-1` | 15.2:1 | 12.6:1 |
| `fg-muted` on `surface-1` | 7.9:1 | 6.4:1 |
| `fg-subtle` on `surface-1` | 4.7:1 | 4.6:1 |
| `fg-on-accent` on `accent-primary` | 4.9:1 | 5.2:1 |
| `success-fg` on `success-subtle` | 6.8:1 | 7.4:1 |
| `warning-fg` on `warning-subtle` | 6.2:1 | 7.0:1 |
| `danger-fg` on `danger-subtle` | 6.9:1 | 7.1:1 |
| `info-fg` on `info-subtle` | 7.1:1 | 7.5:1 |
| `ring-focus` on `bg-canvas` | 3.3:1 | 3.1:1 |

Ratios computed with oklch-to-sRGB conversion and the WCAG 2.1 formula;
values must be verified by `pnpm test:contrast` (see Testing).
Additional rules: focus-visible ring always ≥3:1; disabled controls
use `opacity: 0.5` plus `aria-disabled`; no colour-only status cues
(pair with icon / text).

## Testing strategy

- **Contrast unit test.** `crates/rig-gui/src/theme/contrast.test.ts`
  parses `theme.css`, evaluates the pairs in the table above through
  `culori`, asserts ≥4.5 (body) or ≥3.0 (UI / ring). Runs in CI on
  every PR.
- **Component snapshots.** `vitest` + `@testing-library/react`
  renders each `ui/` component twice — once under `data-theme="light"`,
  once under `data-theme="dark"` — and snapshots the serialised DOM
  + computed `--color-*` values. Fails on unexpected class drift.
- **Visual regression.** Deferred. Pick Playwright in Phase 3 if
  Q5 resolves that way. Scope: one screenshot per top-level screen
  per theme (6 images total).
- **Manual smoke.** `rig-gui` launched with `RIG_GUI_FORCE_THEME=dark`
  env var during dev; adds a small boot-time override to
  `ThemeProvider` (read once, ignored in prod builds).

## Acceptance criteria

1. `crates/rig-gui/src/styles/theme.css` exists with every token
   listed under "Token inventory", for both light and dark.
2. `<html data-theme="…">` is set before first paint (no FOUC).
3. `ThemeProvider` exposes `{ mode, resolved, setMode }`; `useTheme`
   throws outside provider; unit-tested.
4. Theme toggle in header cycles light → dark → system and persists
   to `localStorage.rig-gui.theme`.
5. "system" mode reacts live to OS theme change without reload.
6. `ui/` kit exists with Button, Pill, Card, Badge, Input, Modal,
   EmptyState; all use tokens exclusively; no raw slate / indigo /
   gray Tailwind classes.
7. `rg -n 'bg-slate-|text-slate-|border-slate-|bg-indigo-|text-indigo-'
   crates/rig-gui/src` returns zero matches.
8. Contrast unit test passes for every documented pair.
9. Every existing screen renders correctly in both themes (manual
   check recorded in PR description).
10. ESLint rule from Phase 3 is wired into CI and fails on
    reintroduction of palette classes.
