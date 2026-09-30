import { useEffect, useId, useRef } from "react";

interface ConfirmDialogProps {
  title: string;
  description: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirmLabel?: string;
  destructive?: boolean;
  confirmDisabled?: boolean;
}

export function ConfirmDialog({
  title,
  description,
  onConfirm,
  onCancel,
  confirmLabel = "Confirm",
  destructive = false,
  confirmDisabled = false,
}: ConfirmDialogProps) {
  const id = useId();
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    panel.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => { previous?.focus(); };
  }, []);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div ref={panel} role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-description`}
        onKeyDown={event => {
          if (event.key === "Escape") { event.preventDefault(); onCancel(); }
          if (event.key === "Tab") {
            const buttons = panel.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
            if (!buttons?.length) return;
            const first = buttons[0], last = buttons[buttons.length - 1];
            if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
            if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
          }
        }} className="bg-card border border-border rounded-lg shadow-xl p-6 max-w-lg w-full mx-4 max-h-[85vh] overflow-auto">
        <h3 id={`${id}-title`} className="text-base font-semibold text-card-foreground mb-2">{title}</h3>
        <p id={`${id}-description`} className="text-sm text-muted-foreground mb-5 whitespace-pre-line break-words">{description}</p>
        <div className="flex gap-2 justify-end">
          <button
            onClick={onCancel}
            className="btn"
          >
            Cancel
          </button>
          <button
            disabled={confirmDisabled}
            onClick={onConfirm}
            className={`btn ${
              destructive
                ? "btn-danger"
                : "btn-primary"
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
