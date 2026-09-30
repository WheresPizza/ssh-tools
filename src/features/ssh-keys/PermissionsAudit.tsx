import { useCallback, useEffect, useState } from "react";
import { auditPermissions, fixPermissions, type PermissionIssue } from "../../lib/tauri";
import { useStore } from "../../stores";

export function PermissionsAudit() {
  const [issues, setIssues] = useState<PermissionIssue[] | null>(null);
  const [busy, setBusy] = useState(false);
  const showToast = useStore(s => s.showToast);
  const refresh = useCallback(async () => {
    try { setIssues(await auditPermissions()); }
    catch (error) { showToast(`Permission audit failed: ${error}`, "error"); }
  }, [showToast]);
  useEffect(() => { void refresh(); }, [refresh]);
  async function fix() {
    setBusy(true);
    try { setIssues(await fixPermissions()); showToast("SSH file permissions repaired", "success"); }
    catch (error) { showToast(`Permission repair failed: ${error}`, "error"); }
    finally { setBusy(false); }
  }
  return <section className="mb-5 rounded-lg border border-border p-3 text-sm" aria-label="SSH permissions">
    <div className="flex items-center justify-between gap-3">
      <span>{issues === null ? "Check SSH file permissions" : issues.length ? `${issues.length} files need permission changes` : "SSH file permissions are correct"}</span>
      <button type="button" onClick={refresh} disabled={busy} className="btn underline">Check again</button>
    </div>
    {!!issues?.length && <>
      <ul className="my-3 space-y-1 font-mono text-xs break-all">{issues.map(issue => <li key={issue.path}>{issue.path}: {issue.current} → {issue.expected}</li>)}</ul>
      <button type="button" onClick={fix} disabled={busy} className="btn btn-primary">{busy ? "Repairing…" : "Fix these permissions"}</button>
    </>}
  </section>;
}
