import type { ReactNode } from "react";

interface Props {
  active: boolean;
  onClick?: () => void;
  disabled?: boolean;
  title?: string;
  children: ReactNode;
}

export default function Pill({
  active,
  onClick,
  disabled,
  title,
  children,
}: Props) {
  const base =
    "text-sm px-3 py-1 rounded-md transition-colors focus-visible:outline-none " +
    "focus-visible:ring-2 focus-visible:ring-ring-focus";
  const variant = active
    ? "bg-fg-default text-bg-canvas"
    : "border border-border-default bg-surface-1 text-fg-default hover:bg-surface-2";
  const disabledCls = disabled
    ? "cursor-not-allowed opacity-40 hover:bg-surface-1"
    : "";
  return (
    <button
      onClick={() => !disabled && onClick?.()}
      disabled={disabled}
      title={title}
      className={`${base} ${variant} ${disabledCls}`}
    >
      {children}
    </button>
  );
}
