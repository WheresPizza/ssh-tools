import { useCallback, useEffect, useState } from "react";
import { listBackups, restoreBackup, type BackupInfo } from "../../lib/tauri";
import { subscribe } from "../../lib/events";
import { useStore } from "../../stores";
import { ConfirmDialog } from "./ConfirmDialog";

export function BackupPanel({ onRestored }: { onRestored?: () => void } = {}) {
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [selected, setSelected] = useState<BackupInfo | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const showToast = useStore(s => s.showToast);
  const refresh = useCallback(async () => {
    try { setBackups(await listBackups()); }
    catch (e) { showToast(`Cannot read backups: ${e}`, "error"); }
  }, [showToast]);
  useEffect(() => {
    void refresh();
    const a = subscribe("ssh-config-changed", refresh);
    const b = subscribe("known-hosts-changed", refresh);
    return () => { a(); b(); };
  }, [refresh]);
  async function restore() {
    if (!selected || busy) return;
    setBusy(true);
    try { await restoreBackup(selected); setSelected(null); setConfirm(false); await refresh(); onRestored?.(); showToast("Backup restored; the previous current file is now the backup", "success"); }
    catch (e) { showToast(`Restore failed: ${e}`, "error"); }
    finally { setBusy(false); }
  }
  return <details className="mb-4 rounded border border-border p-3 text-sm">
    <summary className="cursor-pointer">Backups and recovery</summary>
    <p className="my-2 text-muted-foreground">The previous version is saved before each write. Review it before restoring.</p>
    {!backups.length && <p>No backups yet.</p>}
    {backups.map(backup => <button className="btn block text-left underline break-all" key={backup.path} onClick={() => setSelected(backup)}>{backup.path}</button>)}
    {selected && <div className="mt-3">
      <pre className="max-h-56 overflow-auto whitespace-pre-wrap bg-muted p-3 rounded text-xs">{selected.preview}</pre>
      <button className="btn btn-primary mt-2" disabled={busy} onClick={() => setConfirm(true)}>Restore this version</button>
    </div>}
    {confirm && selected && <ConfirmDialog title="Restore backup" description={`Replace ${selected.path} with the reviewed backup? The current version will be retained as the next backup.`} confirmLabel={busy ? "Restoring…" : "Restore"} onConfirm={restore} onCancel={() => { if (!busy) setConfirm(false); }} />}
  </details>;
}
