import { useEffect, useState } from "react";
import { diagnoseSsh, getSshConfig, type DiagnosticReport } from "../../lib/tauri";
import { isConnectableAlias } from "../../lib/ssh-host-utils";
import { useStore } from "../../stores";
export function DiagnosticsPage() {
  const target = useStore(s => s.diagnosticAlias);
  const targetPort = useStore(s => s.diagnosticPort);
  const [port, setPort] = useState<number | null>(targetPort);
  const [alias, setAlias] = useState(target);
  const [aliases, setAliases] = useState<string[]>([]);
  const [report, setReport] = useState<DiagnosticReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => { getSshConfig().then(hosts => setAliases([...new Set(hosts.map(h => h.alias).filter(isConnectableAlias))])).catch(e => setError(String(e))); }, []);
  async function run(network: boolean) {
    if (!isConnectableAlias(alias) || busy) return;
    setBusy(true); setError(null); setReport(null);
    try { setReport(await diagnoseSsh(alias, network, port)); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <section className="space-y-4">
    <header><h2 className="text-lg font-semibold">SSH Diagnostics</h2><p className="text-sm text-muted-foreground">Inspect the settings OpenSSH actually uses, then test public-key authentication.</p></header>
    <div className="flex gap-2"><input list="diagnostic-hosts" className="input-field" aria-label="SSH host to diagnose" placeholder="Choose an SSH alias or enter a hostname" value={alias} disabled={busy} onChange={e => { setAlias(e.target.value); setReport(null); }} /><datalist id="diagnostic-hosts">{aliases.map(value => <option key={value} value={value} />)}</datalist></div>
    <label className="block text-sm max-w-xs">Port override (optional)<input className="input-field mt-1" type="number" min={1} max={65535} disabled={busy} value={port ?? ""} placeholder="Use SSH configuration" onChange={e => { setPort(e.target.value ? Number(e.target.value) : null); setReport(null); }} /></label>
    <div className="flex gap-2"><button disabled={busy || !isConnectableAlias(alias)} className="btn" onClick={() => run(false)}>Check configuration</button><button disabled={busy || !isConnectableAlias(alias)} className="btn btn-primary" onClick={() => run(true)}>Test authentication</button></div>
    <p className="text-xs text-muted-foreground">Authentication contacts the selected server using public keys, with an 8-second limit. It does not open a remote shell, forward ports or accept new host keys. Encrypted keys must already be in the agent. Local configuration checks may resolve hostnames.</p>
    {busy && <p role="status">Checking SSH…</p>}
    {error && <p role="alert" className="border border-destructive rounded p-3">{error}</p>}
    {report && <>
      <div className="border rounded-lg p-4 text-sm space-y-2"><h3 className="font-semibold">{report.alias}</h3><p>{report.user ?? "?"}@{report.hostname ?? "?"}:{report.port ?? "?"} · IdentitiesOnly: {report.identities_only ? "yes" : "no"}</p><p className="text-xs font-mono break-all">{report.config_path}</p>{report.authenticated !== null && <p className={report.authenticated ? "text-success" : "text-destructive"}>{report.authenticated ? "Public-key authentication verified" : "Authentication was not successful"}</p>}</div>
      {report.steps.map((step,i) => <article key={i} className="border rounded-lg p-4 space-y-2"><h3 className="font-medium"><span aria-label={step.status} className={step.status === "ok" ? "text-success" : step.status === "error" ? "text-destructive" : "text-warning"}>{step.status === "ok" ? "✓" : step.status === "error" ? "✕" : "!"}</span> {step.title}</h3><p className="text-sm whitespace-pre-wrap break-all text-muted-foreground">{step.detail}</p>{step.suggestion && <p className="text-sm">{step.suggestion}</p>}</article>)}
    </>}
  </section>;
}
