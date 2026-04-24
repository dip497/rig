import { useEffect, type ReactNode } from "react";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  width?: string;
}

export default function Modal({
  title,
  onClose,
  children,
  width = "w-[560px]",
}: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className={`${width} max-h-[90vh] overflow-auto rounded-md border border-border-default bg-surface-1 p-5 shadow-pop`}
      >
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-fg-default">{title}</h2>
          <button
            onClick={onClose}
            aria-label="Close"
            className="text-fg-subtle hover:text-fg-default rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring-focus"
          >
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
