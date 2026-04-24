import type { ReactNode } from "react";

interface Props {
  title: string;
  description?: string;
  action?: ReactNode;
}

export default function EmptyState({ title, description, action }: Props) {
  return (
    <div className="flex h-full items-center justify-center px-8 py-16">
      <div className="max-w-md text-center">
        <div className="text-base font-medium text-fg-default">{title}</div>
        {description && (
          <div className="mt-1 text-sm text-fg-muted">{description}</div>
        )}
        {action && <div className="mt-4">{action}</div>}
      </div>
    </div>
  );
}
