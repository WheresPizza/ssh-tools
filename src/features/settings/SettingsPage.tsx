import { useCallback, useEffect, useState } from "react";
import { getDetectedTerminal, setPreferredTerminal, type TerminalInfo } from "../../lib/tauri";
import { BackupPanel } from "../../components/common/BackupPanel";

export function SettingsPage() {
  const [terminals, setTerminals] = useState<TerminalInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    setLoading(true);
    try { setTerminals(await getDetectedTerminal()); setError(null); }
    catch (e) { setError(String(e)); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);
  async function selectTerminal(name: string) {
    if (saving) return;
    setSaving(true);
    try { await setPreferredTerminal(name); await refresh(); }
    catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  }
  const preferred = terminals.find(t => t.is_preferred) ?? terminals[0];
  return <section className="space-y-5">
    <header><h2 className="text-lg font-semibold">Settings</h2><p className="text-sm text-muted-foreground">Application preferences and file recovery.</p></header>
    <section aria-labelledby="terminal-heading" className="rounded-lg border bg-card p-4 space-y-3">
      <h3 id="terminal-heading" className="text-base font-semibold">Terminal</h3>
      <p className="text-sm text-muted-foreground">Used for connections from SSH Hosts, passphrase prompts and installing public keys on servers.</p>
      {error && <p role="alert" className="text-destructive break-words">{error}</p>}
      {loading ? <p>Detecting terminals…</p> : <>
        <div className="flex flex-wrap gap-2" role="group" aria-label="Preferred terminal">
          {terminals.map(t => <button key={t.path} disabled={saving} aria-pressed={preferred?.path === t.path} onClick={() => selectTerminal(t.name)} className={`btn ${preferred?.path === t.path ? "border-primary text-primary" : ""}`}>{t.name}</button>)}
          {!terminals.length && <p className="text-muted-foreground">No terminals detected.</p>}
        </div>
        {preferred && <p className="text-xs text-muted-foreground break-all">{preferred.path}</p>}
        <button className="btn btn-compact" disabled={saving} onClick={refresh}>Refresh terminals</button>
      </>}
    </section>
    <section aria-labelledby="recovery-heading" className="space-y-3">
      <h3 id="recovery-heading" className="text-base font-semibold">File recovery</h3>
      <p className="text-sm text-muted-foreground">Previous versions of SSH configuration files and known_hosts. Restoring SSH configuration can also change Git profile aliases. Private key files are not backed up here.</p>
      <BackupPanel />
    </section>
  </section>;
}
