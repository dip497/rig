import type { DriftState } from "../types";
import Badge, { type BadgeColor } from "./Badge";

const COLOR: Record<DriftState, BadgeColor> = {
  clean: "success",
  "local-drift": "warning",
  "upstream-drift": "info",
  "both-drift": "danger",
  missing: "danger",
  orphan: "muted",
};

const LABEL: Record<DriftState, string> = {
  clean: "clean",
  "local-drift": "local drift",
  "upstream-drift": "upstream drift",
  "both-drift": "both drift",
  missing: "missing",
  orphan: "orphan",
};

export default function DriftBadge({ state }: { state: DriftState | null }) {
  if (!state) return <Badge color="muted">—</Badge>;
  return <Badge color={COLOR[state]}>{LABEL[state]}</Badge>;
}
