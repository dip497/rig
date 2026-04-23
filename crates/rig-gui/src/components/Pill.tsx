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
  const base = "text-sm px-3 py-1 rounded transition";
  const variant = active
    ? "bg-slate-900 text-white"
    : "border border-slate-200 bg-white text-slate-700 hover:bg-slate-50";
  const disabledCls = disabled ? "cursor-not-allowed opacity-40 hover:bg-white" : "";
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
