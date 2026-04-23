import type { HTMLAttributes, ReactNode } from "react";

interface Props extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
}

export default function Card({ children, className = "", ...rest }: Props) {
  return (
    <div
      className={`bg-surface-1 border border-border-default rounded-md p-4 ${className}`}
      {...rest}
    >
      {children}
    </div>
  );
}
