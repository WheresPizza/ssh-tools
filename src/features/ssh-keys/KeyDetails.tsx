import { useEffect, useRef, useState } from "react";
import { getKeyUsage, saveKeyMetadata, type KeyMetadata, type KeyUsage, type MetadataSnapshot, type SshKeyInfo } from "../../lib/tauri";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";

export const emptyMetadata: KeyMetadata = { tags: [], purpose: "", note: "", replace_on: null };
export function replacementDue(date: string | null): boolean {
  if (!date) return false;
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  return date <= today;
}

export function UsageSummary({ usage }: { usage: KeyUsage }) {
  return <div className="space-y-3 text-sm">
    {([['Hosts', usage.hosts], ['Git profiles', usage.profiles], ['Repositories', usage.repositories]] as const).map(([label, values]) => <div key={label}>
      <h4 className="font-semibold">{label}</h4>
      {values.length ? <ul className="mt-1 space-y-1 text-muted-foreground break-words">{[...new Set(values)].map(v => <li key={v}>{v}</li>)}</ul> : <p className="text-muted-foreground">No local links found.</p>}
    </div>)}
    <p className="text-xs text-muted-foreground break-words">{usage.roots.length ? `Checked project folders: ${usage.roots.join(", ")}` : "No project folders selected; repository usage has not been checked. Choose folders in Repositories."}</p>
    {usage.warnings.map((w, i) => <p key={i} role="status" className="text-warning text-xs break-words">{w}</p>)}
    <p className="text-xs text-muted-foreground">Local references are configuration evidence. Unscanned repositories, dynamic SSH rules and remote installations are outside this check.</p>
  </div>;
}

export function KeyDetails({ keyInfo, copies, snapshot, metadataError, agentStatus, onSaved, onReload, onClose }: {
  keyInfo: SshKeyInfo; copies: SshKeyInfo[]; snapshot: MetadataSnapshot | null; metadataError: string | null;
  agentStatus: string; onSaved: (s: MetadataSnapshot) => void; onReload: () => Promise<void>; onClose: () => void;
}) {
  // Keep the opening revision and draft stable across background refreshes.
  const [base, setBase] = useState(snapshot);
  const [draft, setDraft] = useState<KeyMetadata>(() => snapshot?.entries[keyInfo.fingerprint] ?? emptyMetadata);
  const [tags, setTags] = useState(draft.tags.join(", "));
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [discard, setDiscard] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [usage, setUsage] = useState<KeyUsage | null>(null);
  const [usageError, setUsageError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    if (!dirty && !saving && snapshot) {
      setBase(snapshot);
      const value = snapshot.entries[keyInfo.fingerprint] ?? emptyMetadata;
      setDraft(value); setTags(value.tags.join(", "));
    }
  }, [snapshot, dirty, saving, keyInfo.fingerprint]);
  useEffect(() => {
    let active = true;
    setUsage(null); setUsageError(null);
    getKeyUsage(keyInfo.private_path).then(u => { if (active) setUsage(u); }).catch(e => { if (active) setUsageError(String(e)); });
    return () => { active = false; };
  }, [keyInfo.private_path, refresh]);
  const change = (value: Partial<KeyMetadata>) => { setDraft(d => ({ ...d, ...value })); setDirty(true); };
  const save = async () => {
    if (!base || saving || keyInfo.error) return;
    setSaving(true); setError(null);
    try {
      const saved = await saveKeyMetadata(keyInfo.private_path, keyInfo.fingerprint, { ...draft, tags: tags.split(",") }, base.revision);
      onSaved(saved); setBase(saved); setDirty(false);
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };
  return <section className="space-y-4">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div><h2 ref={heading} tabIndex={-1} className="text-lg font-semibold break-all">{keyInfo.name}</h2><p className="text-sm text-muted-foreground">Key details</p></div>
      <button className="btn" disabled={saving} onClick={() => dirty ? setDiscard(true) : onClose()}>Back to keys</button>
    </div>
    {keyInfo.error && <p role="alert" className="text-destructive break-words">{keyInfo.error}</p>}
    <div className="rounded-lg border border-border bg-card p-4 space-y-3">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
        <dt className="text-muted-foreground">Fingerprint</dt><dd className="break-all select-text">{keyInfo.fingerprint || "Unavailable"}</dd>
        <dt className="text-muted-foreground">Algorithm</dt><dd>{keyInfo.algorithm}{keyInfo.bits ? ` · ${keyInfo.bits} bits` : ""}</dd>
        <dt className="text-muted-foreground">Private file</dt><dd className="break-all select-text">{keyInfo.private_path}</dd>
        <dt className="text-muted-foreground">Public file</dt><dd className="break-all select-text">{keyInfo.public_path}{keyInfo.public_key_exists === false ? " (missing)" : ""}</dd>
        <dt className="text-muted-foreground">Passphrase</dt><dd>{keyInfo.has_passphrase == null ? "Unknown" : keyInfo.has_passphrase ? "Protected" : "Not set"}</dd>
        <dt className="text-muted-foreground">Agent</dt><dd>{agentStatus}</dd>
        <dt className="text-muted-foreground">Comment</dt><dd className="break-words">{keyInfo.comment || "—"}</dd>
        <dt className="text-muted-foreground">File modified</dt><dd>{keyInfo.created_at || "Unknown"}</dd>
      </dl>
      <h3 className="text-sm font-semibold">Files with this fingerprint ({copies.length})</h3>
      <ul className="text-xs text-muted-foreground space-y-1 break-all">{copies.map(k => <li key={k.private_path}>{k.private_path}</li>)}</ul>
      <p className="text-xs text-muted-foreground">Copies share an identity and annotations. File actions and references remain specific to each path.</p>
    </div>
    <form className="rounded-lg border border-border bg-card p-4 space-y-3" onSubmit={e => { e.preventDefault(); void save(); }}>
      <h3 className="text-sm font-semibold">Labels & notes</h3>
      <p className="text-xs text-muted-foreground">Stored locally by fingerprint, separately from key files. Do not store passwords or private key material here.</p>
      {metadataError && <p role="alert" className="text-destructive break-words">{metadataError}</p>}
      {error && <p role="alert" className="text-destructive break-words">{error} Your edits are still in the form. Copy them before reloading.</p>}
      {keyInfo.error && <p className="text-xs text-muted-foreground">Annotations are unavailable until this key can be read and its identity verified.</p>}
      <fieldset disabled={!base || saving || !!keyInfo.error || !!metadataError} className="space-y-3 min-w-0">
        <label className="block text-sm">Tags <span className="text-muted-foreground">(comma separated)</span><input className="input-field w-full mt-1" value={tags} onChange={e => { setTags(e.target.value); setDirty(true); }} placeholder="work, client-acme" /></label>
        <label className="block text-sm">Purpose<input className="input-field w-full mt-1" maxLength={256} value={draft.purpose} onChange={e => change({ purpose: e.target.value })} /></label>
        <label className="block text-sm">Notes<textarea className="input-field w-full mt-1 min-h-24" maxLength={4096} value={draft.note} onChange={e => change({ note: e.target.value })} /></label>
        <label className="block text-sm">Planned replacement<input type="date" className="input-field block mt-1" min="0001-01-01" max="9999-12-31" value={draft.replace_on ?? ""} onChange={e => change({ replace_on: e.target.value || null })} /></label>
        <p className="text-xs text-muted-foreground">A local reminder, not a key expiration date. {replacementDue(draft.replace_on) ? "Replacement is due." : ""}</p>
        <button type="submit" disabled={!dirty} className="btn btn-primary">{saving ? "Saving…" : "Save annotations"}</button>
      </fieldset>
      {(error || metadataError || !base) && <button type="button" className="btn" disabled={saving || dirty} onClick={() => { setError(null); void onReload(); }}>Reload annotations</button>}
      {dirty && <button type="button" className="btn ml-2" disabled={saving} onClick={() => setDiscard(true)}>Discard edits</button>}
    </form>
    <div className="rounded-lg border border-border bg-card p-4 space-y-3">
      <div className="flex justify-between gap-3"><h3 className="text-sm font-semibold">References to this file</h3><button className="btn btn-compact" onClick={() => setRefresh(r => r + 1)}>Recheck references</button></div>
      {usage ? <UsageSummary usage={usage} /> : <p role="status" className="text-sm text-muted-foreground break-words">{usageError || "Checking local configuration and project folders…"}</p>}
    </div>
    {discard && <ConfirmDialog title="Discard annotation edits?" description="Your unsaved edits will be lost. Existing annotations and key files will stay unchanged." confirmLabel="Discard edits" destructive onCancel={() => setDiscard(false)} onConfirm={onClose} />}
  </section>;
}
