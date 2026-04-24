import type { ReactNode } from "react";

export type BadgeColor =
  | "neutral"
  | "muted"
  | "success"
  | "warning"
  | "danger"
  | "info"
  | "accent";

const COLORS: Record<BadgeColor, string> = {
  neutral: "bg-surface-2 text-fg-default",
  muted: "bg-surface-2 text-fg-muted",
  success: "bg-success-subtle text-success-fg",
  warning: "bg-warning-subtle text-warning-fg",
  danger: "bg-danger-subtle text-danger-fg",
  info: "bg-info-subtle text-info-fg",
  accent: "bg-accent-subtle text-accent-primary",
};

interface Props {
  color?: BadgeColor;
  className?: string;
  title?: string;
  children: ReactNode;
}

export default function Badge({
  color = "neutral",
  className = "",
  title,
  children,
}: Props) {
  return (
    <span
      title={title}
      className={`inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium ${COLORS[color]} ${className}`}
    >
      {children}
    </span>
  );
}
