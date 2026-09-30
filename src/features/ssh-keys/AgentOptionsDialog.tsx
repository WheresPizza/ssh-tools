import { useState } from "react";
import type { AgentOptions } from "../../lib/tauri";
export function AgentOptionsDialog({ keyName, onConfirm, onCancel }: { keyName: string; onConfirm: (options: AgentOptions) => void; onCancel: () => void }) {
  const [duration, setDuration] = useState("3600");
  const [end, setEnd] = useState("18:00");
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  function submit(event: React.FormEvent) {
    event.preventDefault();
    let seconds = duration === "none" ? null : Number(duration);
    if (duration === "end") {
      const [hours, minutes] = end.split(":").map(Number); const target = new Date(); target.setHours(hours, minutes, 0, 0);
      seconds = Math.floor((target.getTime() - Date.now()) / 1000);
      if (!Number.isFinite(seconds) || seconds < 1) { setError("Choose an end time later today."); return; }
    }
    onConfirm({ lifetime_seconds: seconds, confirm });
  }
  return <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center"><form onSubmit={submit} role="dialog" aria-modal="true" aria-labelledby="agent-title" className="bg-card border rounded-lg p-6 max-w-md w-full mx-4 space-y-4">
    <h3 id="agent-title" className="font-semibold">Add {keyName} to SSH agent</h3>
    <label className="block text-sm">Keep key available<select className="input-field mt-1" value={duration} onChange={e => setDuration(e.target.value)}><option value="3600">1 hour</option><option value="28800">8 hours</option><option value="end">Until a time today</option><option value="none">Agent default lifetime</option></select></label>
    {duration === "end" && <label className="block text-sm">End of workday (local time)<input className="input-field mt-1" type="time" required value={end} onChange={e => setEnd(e.target.value)} /></label>}
    <label className="flex gap-2 items-start text-sm"><input type="checkbox" checked={confirm} onChange={e => setConfirm(e.target.checked)} />Require confirmation for every use</label>
    {confirm && <p className="text-xs text-muted-foreground">Your SSH agent must have a working confirmation helper (SSH_ASKPASS). Without it, signing requests are refused; they are never silently approved.</p>}
    <p className="text-xs text-muted-foreground">The SSH agent enforces the time limit even when this app is closed. Encrypted keys ask for their passphrase in a terminal.</p>
    {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
    <div className="flex gap-2"><button className="btn btn-primary" type="submit">Add with these settings</button><button className="btn" type="button" onClick={onCancel}>Cancel</button></div>
  </form></div>;
}
