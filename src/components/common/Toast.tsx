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
    <div
      className={cn(
        "fixed bottom-4 right-4 z-50 px-4 py-3 rounded-lg shadow-lg text-sm font-medium max-w-sm",
        "animate-in slide-in-from-bottom-4 duration-200",
        type === "success" && "bg-green-500 text-white",
        type === "error" && "bg-destructive text-destructive-foreground",
        type === "info" && "bg-primary text-primary-foreground"
      )}
    >
      <div className="flex items-start gap-2">
        <span className="flex-1">{message}</span>
        <button onClick={onClose} className="opacity-70 hover:opacity-100 ml-2 shrink-0">✕</button>
      </div>
    </div>
  );
}
