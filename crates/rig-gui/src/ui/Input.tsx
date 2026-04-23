import { forwardRef, type InputHTMLAttributes } from "react";

type Props = InputHTMLAttributes<HTMLInputElement>;

const base =
  "rounded-md border border-border-default bg-surface-1 text-fg-default " +
  "px-2 py-1 text-sm placeholder:text-fg-subtle " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring-focus " +
  "disabled:opacity-50";

const Input = forwardRef<HTMLInputElement, Props>(
  ({ className = "", ...rest }, ref) => (
    <input ref={ref} className={`${base} ${className}`} {...rest} />
  ),
);
Input.displayName = "Input";

export default Input;
