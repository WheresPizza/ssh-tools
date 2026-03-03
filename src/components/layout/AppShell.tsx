import { Sidebar } from "./Sidebar";
import { Toast } from "../common/Toast";
import { useStore } from "../../stores";

interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps) {
  const toast = useStore((s) => s.toast);
  const clearToast = useStore((s) => s.clearToast);

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <Sidebar />
      <main className="flex-1 overflow-auto p-6">
        {children}
      </main>
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={clearToast}
        />
      )}
    </div>
  );
}
