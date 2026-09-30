import { useEffect, useState } from "react";
import { auditSshKeys, type KeyAudit, type MetadataSnapshot } from "../../lib/tauri";
import { replacementDue, UsageSummary } from "./KeyDetails";

export function KeyAuditPanel({ metadata, revision, onOpen, onClose }: { metadata: MetadataSnapshot | null; revision: number; onOpen: (path: string) => void; onClose: () => void }) {
  const [report, setReport] = useState<KeyAudit | null>(null);
  const [checkedRevision, setCheckedRevision] = useState(-1);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("all");
  const [run, setRun] = useState(0);
  useEffect(() => {
    let active = true;
    setLoading(true); setError(null);
    auditSshKeys().then(r => { if (active) { setReport(r); setCheckedRevision(revision); } }).catch(e => { if (active) setError(String(e)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
    // A scan is explicit; filesystem events only mark the snapshot stale.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run]);
  const entries = report?.entries.map(entry => {
    const date = metadata?.entries[entry.fingerprint]?.replace_on;
    return { ...entry, findings: [...entry.findings, ...(date && replacementDue(date) ? [{ code: "replacement-due", severity: "warning" as const, title: "Planned replacement is due", detail: `Your planned date was ${date}. This does not expire or revoke the key.` }] : [])] };
  }) ?? [];
  const count = (severity: string) => entries.reduce((n, e) => n + e.findings.filter(f => f.severity === severity).length, 0);
  return <section className="border border-border rounded-lg bg-card p-4 mb-4 space-y-3" aria-label="Local key audit">
    <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="font-semibold">Local key audit</h3><div className="flex gap-2"><button className="btn btn-compact" disabled={loading} onClick={() => setRun(n => n + 1)}>{loading ? "Checking…" : "Run again"}</button><button className="btn btn-compact" onClick={onClose}>Close audit</button></div></div>
    <p className="text-xs text-muted-foreground">Read-only checks of key files, permissions and local references. No server connections or automatic fixes.</p>
    {error && <p role="alert" className="text-destructive break-words">{error}</p>}
    {loading && <p role="status" className="text-sm">Checking key files and project folders…</p>}
    {report && <>
      <p className="text-xs text-muted-foreground">Checked {new Date(report.checked_at * 1000).toLocaleString()} · {entries.length} files · {count("error")} errors · {count("warning")} warnings · {count("info")} notes</p>
      {checkedRevision !== revision && <p role="status" className="text-warning text-sm">Local data changed. Run the audit again for current findings.</p>}
      {!metadata && <p role="status" className="text-warning text-xs">Annotations are unavailable; planned replacement dates were not checked.</p>}
      {report.warnings.map((w, i) => <p role="status" className="text-warning text-xs break-words" key={i}>{w}</p>)}
      <label className="text-sm flex items-center gap-2">Findings<select className="input-field" value={filter} onChange={e => setFilter(e.target.value)}><option value="all">All files</option><option value="error">Errors</option><option value="warning">Warnings</option><option value="info">Notes</option></select></label>
      {entries.filter(e => filter === "all" || e.findings.some(f => f.severity === filter)).map(entry => <article key={entry.key_path} className="border-t border-border pt-3 space-y-2">
        <div className="flex flex-wrap justify-between items-center gap-2"><span className="text-sm break-all">{entry.key_path}</span><button className="btn btn-compact" onClick={() => onOpen(entry.key_path)}>Key details</button></div>
        {entry.findings.length === 0 && <p className="text-xs text-muted-foreground">No findings from completed checks.</p>}
        {entry.findings.filter(f => filter === "all" || f.severity === filter).map((f, i) => <div key={`${f.code}-${i}`} className="text-sm"><p className={f.severity === "error" ? "text-destructive" : f.severity === "warning" ? "text-warning" : "text-muted-foreground"}>{f.severity} · {f.title}</p><p className="text-xs text-muted-foreground break-words">{f.detail}</p></div>)}
        {entry.usage && <details className="text-xs"><summary className="cursor-pointer">Local references and scan coverage</summary><div className="mt-3"><UsageSummary usage={entry.usage} /></div></details>}
      </article>)}
      {entries.filter(e => filter === "all" || e.findings.some(f => f.severity === filter)).length === 0 && <p className="text-sm text-muted-foreground">{entries.length ? "No findings match this filter." : "No key files found."}</p>}
    </>}
  </section>;
}
