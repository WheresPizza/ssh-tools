import { useEffect } from "react";
import { cn } from "../../lib/utils";

interface ToastProps {
  message: string;
  type: "success" | "error" | "info";
  onClose: () => void;
}

export function Toast({ message, type, onClose }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(onClose, 3000);
    return () => clearTimeout(timer);
  }, [onClose]);

  return (
    <div role={type === "error" ? "alert" : "status"}
      className={cn(
        "fixed bottom-4 right-4 z-50 px-4 py-3 rounded-md border bg-card shadow-lg text-sm max-w-sm",

        type === "success" && "border-success text-success",
        type === "error" && "border-destructive text-destructive",
        type === "info" && "border-primary text-foreground"
      )}
    >
      <div className="flex items-start gap-2">
        <span className="flex-1">{message}</span>
        <button onClick={onClose} aria-label="Dismiss notification" className="opacity-70 hover:opacity-100 ml-2 shrink-0">✕</button>
      </div>
    </div>
  );
}
