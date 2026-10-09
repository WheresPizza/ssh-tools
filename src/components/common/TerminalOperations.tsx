import { useEffect, useRef, useState } from "react";
import { dismissTerminalOperation, listTerminalOperations } from "../../lib/tauri";
import { isPending, useOperations } from "../../lib/operations";

export function TerminalOperations() {
  const operations = useOperations(s => s.operations);
  const [error, setError] = useState<string | null>(null);
  const [dismissing, setDismissing] = useState<string | null>(null);
  const refreshRef = useRef<() => void>(() => {});
  useEffect(() => {
    let disposed = false, running = false;
    const refresh = async () => {
      if (running) return;
      running = true;
      try {
        const next = await listTerminalOperations();
        if (disposed) return;
        const previous = useOperations.getState().operations;
        useOperations.getState().replace(next);
        setError(null);
        if (next.some(op => op.kind === "agent_add" && !isPending(op) && previous.find(p => p.id === op.id)?.state !== op.state)) {
          window.dispatchEvent(new Event("ssh-agent-changed"));
        }
      } catch (e) { if (!disposed) setError(`Cannot check terminal completion: ${e}`); }
      finally { running = false; }
    };
    refreshRef.current = () => { void refresh(); };
    void refresh();
    const interval = window.setInterval(refresh, 1500);
    window.addEventListener("focus", refresh);
    window.addEventListener("terminal-operation-started", refresh);
    return () => { disposed = true; window.clearInterval(interval); window.removeEventListener("focus", refresh); window.removeEventListener("terminal-operation-started", refresh); };
  }, []);
  async function dismiss(id: string) {
    if (dismissing) return;
    setDismissing(id);
    try { await dismissTerminalOperation(id); refreshRef.current(); }
    catch (e) { setError(String(e)); }
    finally { setDismissing(null); }
  }
  if (!operations.length && !error) return null;
  return <section aria-label="Terminal operations" className="mb-4 space-y-2 max-h-52 overflow-auto">
    {error && <p role="alert" className="border border-destructive rounded p-3 text-sm">{error} <button className="btn btn-compact" onClick={() => refreshRef.current()}>Retry status</button></p>}
    {operations.map(op => <div key={op.id} role={op.state === "failed" ? "alert" : "status"} className={`rounded border p-3 text-sm ${op.state === "failed" ? "border-destructive" : "border-border"}`}>
      <div className="flex justify-between gap-2"><strong>{op.kind === "agent_add" ? "Add to agent" : "Install public key"} · {op.state.replace(/_/g, " ")}</strong>
        {!isPending(op) && <button className="btn btn-compact" disabled={!!dismissing} onClick={() => void dismiss(op.id)}>Dismiss result</button>}</div>
      <p className="break-all text-xs text-muted-foreground">{op.target}</p><p>{op.detail}</p>
    </div>)}
  </section>;
}
