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
    "bg-accent-primary text-fg-on-accent shadow-sm hover:bg-accent-primary-hover",
  secondary:
    "bg-surface-1 text-fg-default border border-border-default shadow-sm hover:bg-surface-2",
  ghost:
    "bg-transparent text-fg-muted hover:bg-surface-2 hover:text-fg-default",
  danger:
    "bg-surface-1 text-danger-fg border border-danger/40 shadow-sm hover:bg-danger-subtle",
};

const sizes: Record<Size, string> = {
  sm: "h-7 px-2 text-[12px]",
  md: "h-8 px-3 text-[13px]",
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

export default Button;
