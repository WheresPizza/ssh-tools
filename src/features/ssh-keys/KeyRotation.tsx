import { useEffect, useRef, useState } from "react";
import { applyKeyRotation, listSshKeys, previewKeyRotation, type KeyRotationPlan, type SshKeyInfo } from "../../lib/tauri";

export function KeyRotation({ keyInfo, onChanged, onBusy }: { keyInfo: SshKeyInfo; onChanged: () => void; onBusy: (busy: boolean) => void }) {
  const [opened, setOpened] = useState(false);
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [newPath, setNewPath] = useState("");
  const [plan, setPlan] = useState<KeyRotationPlan | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [step, setStep] = useState<"choose" | "select" | "review" | "done">("choose");
  const [registered, setRegistered] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const locked = useRef(false);
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!opened) return;
    const previous = document.activeElement as HTMLElement | null;
    panel.current?.focus();
    return () => previous?.focus();
  }, [opened]);
  async function run(action: () => Promise<void>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); onBusy(true); setError(null);
    try { await action(); } catch (e) { setError(String(e)); }
    finally { locked.current = false; setBusy(false); onBusy(false); }
  }
  function start() {
    setOpened(true); setStep("choose"); setPlan(null); setSelected([]); setRegistered(false); setNewPath(""); setKeys([]);
    void run(async () => setKeys((await listSshKeys()).filter(k => !k.error && k.fingerprint !== keyInfo.fingerprint && k.private_path !== keyInfo.private_path)));
  }
  function close() { if (!locked.current) setOpened(false); }
  async function preview() {
    const replacement = keys.find(k => k.private_path === newPath);
    if (!replacement) return;
    await run(async () => {
      const value = await previewKeyRotation(keyInfo, replacement);
      setPlan(value); setSelected([]); setRegistered(false); setStep("select");
    });
  }
  const chosen = plan?.targets.filter(t => selected.includes(t.id)) ?? [];
  return <>
    <button className="btn" disabled={!!keyInfo.error || busy} onClick={start}>Replace key references</button>
    {opened && <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div ref={panel} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="key-rotation-title" className="w-full max-w-2xl max-h-[85vh] overflow-auto rounded-lg border border-border bg-card p-5 space-y-4" onKeyDown={e => {
        if (e.key === "Escape") { e.preventDefault(); close(); }
        if (e.key === "Tab") {
          const items = panel.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled)');
          if (!items?.length) { e.preventDefault(); return; }
          const first = items[0], last = items[items.length - 1];
          if (e.shiftKey && (document.activeElement === first || document.activeElement === panel.current)) { e.preventDefault(); last.focus(); }
          else if (!e.shiftKey && (document.activeElement === last || document.activeElement === panel.current)) { e.preventDefault(); first.focus(); }
        }
      }}>
        <header className="flex flex-wrap items-center justify-between gap-2"><h3 id="key-rotation-title" className="text-base font-semibold">Replace key references</h3><button className="btn" disabled={busy} onClick={close}>{step === "done" ? "Close" : "Cancel"}</button></header>
        <p className="text-xs text-muted-foreground break-all">Current key: {keyInfo.name} · {keyInfo.fingerprint}</p>
        {error && <p role="alert" className="text-sm text-destructive break-words">{error}</p>}
        {step === "choose" && <div className="space-y-3">
          <p className="text-sm">1. Choose a different key from your library. Generate or import it in SSH Keys first if needed.</p>
          <label className="block text-sm">Replacement key<select className="input-field mt-1" value={newPath} disabled={busy} onChange={e => setNewPath(e.target.value)}><option value="">Choose a key…</option>{keys.map(k => <option key={k.private_path} value={k.private_path}>{k.name} · {k.fingerprint}</option>)}</select></label>
          {!busy && !keys.length && <p className="text-sm text-muted-foreground">No other usable key identities in this library.</p>}
          {error && <button className="btn" disabled={busy} onClick={start}>Reload keys</button>}
          <button className="btn btn-primary" disabled={!newPath || busy} onClick={() => void preview()}>{busy ? "Checking…" : "Find references"}</button>
        </div>}
        {plan && step === "select" && <div className="space-y-3">
          <p className="text-sm">2. Select the references to switch. Nothing is selected automatically.</p>
          {!plan.targets.length && <p className="text-sm">No explicit references to this key file were found.</p>}
          {plan.targets.map(t => <label key={t.id} className="flex gap-3 rounded border border-border p-3 text-sm">
            <input type="checkbox" className="mt-1 shrink-0" disabled={busy || !!t.blocked} checked={selected.includes(t.id)} onChange={e => setSelected(ids => e.target.checked ? [...ids, t.id] : ids.filter(id => id !== t.id))} />
            <span className="min-w-0"><span className="block break-words">{t.kind === "profile" ? "Git profile" : "SSH host"}: {t.label}</span><span className="block text-xs text-muted-foreground break-all">{t.source}{t.line ? `:${t.line}` : ""}</span>{t.blocked && <span className="block text-xs text-warning">{t.blocked}</span>}</span>
          </label>)}
          {plan.warnings.map((w,i) => <p key={i} className="text-xs text-warning break-words">{w}</p>)}
          <div className="flex flex-wrap gap-2"><button className="btn" disabled={busy} onClick={() => setStep("choose")}>Change replacement key</button><button className="btn btn-primary" disabled={busy || !selected.length} onClick={() => { setRegistered(false); setStep("review"); }}>Review {selected.length} selected references</button></div>
        </div>}
        {plan && step === "review" && <div className="space-y-3">
          <p className="text-sm">3. Review changes</p>
          <p className="text-xs break-all">New identity: {plan.new_fingerprint}<br />{plan.new_path}</p>
          {chosen.map(t => <div className="rounded border border-border p-3 space-y-1 text-xs" key={t.id}><p className="font-semibold break-words">{t.label}</p><p className="text-muted-foreground break-all">{t.source}{t.line ? `:${t.line}` : ""}</p><p className="break-all">Before: <code>{t.before}</code></p><p className="break-all">After: <code>{t.after}</code></p></div>)}
          <p className="text-xs text-muted-foreground">4. Apply with a previous-version backup for each changed file. Recovery is in Settings → File recovery. Old key files and agent membership remain unchanged; profile aliases and repository URLs stay the same.</p>
          <label className="flex items-start gap-2 text-sm"><input type="checkbox" className="mt-1" checked={registered} disabled={busy} onChange={e => setRegistered(e.target.checked)} /><span>I have registered the new public key with the affected services.</span></label>
          <div className="flex flex-wrap gap-2"><button className="btn" disabled={busy} onClick={() => setStep("select")}>Back to selection</button><button className="btn btn-primary" disabled={busy || !registered} onClick={() => void run(async () => { await applyKeyRotation(plan, selected); setStep("done"); onChanged(); })}>{busy ? "Applying…" : "Apply replacement"}</button></div>
          {error && <button className="btn" disabled={busy} onClick={() => void preview()}>Refresh replacement plan</button>}
        </div>}
        {step === "done" && <div role="status" className="space-y-2 text-sm"><p>Selected references now use the new key.</p><p>The old key is still in your library. Check authentication before deciding whether to retire it. Removing a local key does not revoke remote access.</p><p className="text-xs text-muted-foreground">Annotations and planned replacement reminders were not transferred or cleared.</p></div>}
      </div>
    </div>}
  </>;
}
