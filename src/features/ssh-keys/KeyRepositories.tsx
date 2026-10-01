import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { applyRepositoryAccess, inspectRepository, listGitProfiles, previewRepositoryAccess, setRepositoryRoots, type GitProfile, type KeyUsage, type RepositoryAccessPlan, type RepositoryScan, type SshKeyInfo } from "../../lib/tauri";

/** Folder coverage is shared by all keys; access setup always targets this key. */
export function KeyRepositories({ keyInfo, usage, onChanged }: { keyInfo: SshKeyInfo; usage: KeyUsage | null; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [selected, setSelected] = useState<RepositoryScan | null>(null);
  const [profiles, setProfiles] = useState<GitProfile[]>([]);
  const [remote, setRemote] = useState("");
  const [alias, setAlias] = useState("");
  const [plan, setPlan] = useState<RepositoryAccessPlan | null>(null);
  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setError(null); setMessage(null);
    try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function rootsChanged(roots: string[]) {
    await setRepositoryRoots(roots);
    // Refresh references even if discovery subsequently fails.
    onChanged();
  }
  function addFolder() {
    void run(async () => {
      const value = await open({ directory: true, multiple: true, title: "Choose folders to check key usage" });
      if (value) await rootsChanged([...new Set([...(usage?.roots ?? []), ...(Array.isArray(value) ? value : [value])])]);
    });
  }
  function chooseRepository() {
    void run(async () => {
      const value = await open({ directory: true, multiple: false, title: "Choose a Git repository for this key" });
      if (!value || Array.isArray(value)) return;
      setSelected(null); setPlan(null);
      const [found, snapshot] = await Promise.all([inspectRepository(value), listGitProfiles()]);
      const available = snapshot.profiles.filter(p => p.key_path === keyInfo.private_path);
      setProfiles(available); setAlias(available[0]?.alias ?? "");
      setSelected(found); setRemote(found.remotes[0]?.name ?? "");
    });
  }
  return <section className="rounded-lg border border-border bg-card p-4 space-y-4" aria-label="Repository access">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <h3 className="text-sm font-semibold">Repository access</h3>
      <button className="btn" disabled={busy || !!keyInfo.error} onClick={chooseRepository}>Set up repository access</button>
    </div>
    <p className="text-xs text-muted-foreground">Use this key through one of its Git profile aliases. Review the remote URL before applying. The public key must already be registered with your Git provider.</p>
    {error && <p role="alert" className="text-destructive text-sm break-words">{error}</p>}
    {message && <p role="status" className="text-sm break-words">{message}</p>}
    {selected && <div className="rounded border border-border p-3 space-y-3 text-sm">
      <p className="break-all">Repository: {selected.roots[0]}</p>
      {!profiles.length ? <p>Create an alias for this key in Git Profiles, then choose the repository again.</p> : <>
        <label className="block">Remote<select className="input-field mt-1" disabled={busy} value={remote} onChange={e => { setRemote(e.target.value); setPlan(null); }}>{[...new Set(selected.remotes.map(r => r.name))].map(name => <option key={name}>{name}</option>)}</select></label>
        <label className="block">Profile for this key<select className="input-field mt-1" disabled={busy} value={alias} onChange={e => { setAlias(e.target.value); setPlan(null); }}>{profiles.map(p => <option key={p.alias} value={p.alias}>{p.name} · {p.alias}</option>)}</select></label>
        <button className="btn" disabled={busy || !remote || !alias} onClick={() => void run(async () => { setPlan(null); setPlan(await previewRepositoryAccess(selected.roots[0], remote, keyInfo, alias)); })}>Preview URL change</button>
      </>}
      {!selected.remotes.length && <p>No remote URLs found in this repository.</p>}
      {selected.warnings.map((w, i) => <p className="text-warning break-words" key={i}>{w}</p>)}
      {plan && <div className="space-y-2 border-t border-border pt-3">
        <p className="break-all">Current: <code>{plan.before}</code></p>
        <p className="break-all">Proposed: <code>{plan.after}</code></p>
        {plan.warnings.map(w => <p key={w} className="text-xs text-warning">{w}</p>)}
        <p className="text-xs text-muted-foreground">A copy of the previous config is saved as .git/.config.ssh-gui.bak. This folder is not automatically added to usage scans.</p>
        <button className="btn btn-primary" disabled={busy || plan.before === plan.after} onClick={() => void run(async () => {
          await applyRepositoryAccess(plan);
          setSelected(null); setPlan(null); onChanged();
          setMessage("Repository URL updated. Authentication has not been tested.");
        })}>Apply URL change</button>
        {plan.before === plan.after && <p>This remote already uses the selected alias.</p>}
      </div>}
      <button className="btn" disabled={busy} onClick={() => { setSelected(null); setPlan(null); }}>Close setup</button>
    </div>}
    <details>
      <summary className="cursor-pointer text-sm font-semibold">Project folders</summary>
      <div className="space-y-3 pt-3 text-sm">
        <p className="text-xs text-muted-foreground">Shared scan coverage for all keys, the audit and deletion checks. Scanning only reads configuration; removing a folder here keeps its files.</p>
        {!usage && <p>{error ? "Could not load project folders." : "Loading project folders…"}</p>}
        {usage?.roots.map(root => <div className="flex items-center gap-2" key={root}><code className="flex-1 break-all text-xs">{root}</code><button className="btn shrink-0" disabled={busy} onClick={() => void run(() => rootsChanged(usage.roots.filter(r => r !== root)))}>Stop scanning</button></div>)}
        <div className="flex flex-wrap gap-2"><button className="btn" disabled={busy || !usage} onClick={addFolder}>Add project folder</button></div>
        {usage?.roots.length === 0 && <p className="text-muted-foreground">No folders added to scan coverage.</p>}
      </div>
    </details>

  </section>;
}
