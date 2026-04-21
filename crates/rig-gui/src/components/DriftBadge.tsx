import type { DriftState } from "../types";

const COLORS: Record<DriftState, string> = {
  clean: "bg-green-100 text-green-800",
  "local-drift": "bg-yellow-100 text-yellow-800",
  "upstream-drift": "bg-blue-100 text-blue-800",
  "both-drift": "bg-orange-100 text-orange-800",
  missing: "bg-red-100 text-red-800",
  orphan: "bg-gray-200 text-gray-800",
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
  if (!state) {
    return (
      <span className="inline-block rounded px-2 py-0.5 text-xs font-medium bg-gray-100 text-gray-600">
        —
      </span>
    );
  }
  return (
    <span
      className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${COLORS[state]}`}
    >
      {LABEL[state]}
    </span>
  );
}
