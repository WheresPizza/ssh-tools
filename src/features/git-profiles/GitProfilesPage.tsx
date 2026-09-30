import { useCallback, useEffect, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { listGitProfiles, saveGitProfile, deleteGitProfile, listSshKeys, type GitProfile, type ProfileSnapshot, type SshKeyInfo } from "../../lib/tauri";
import { subscribe } from "../../lib/events";
import { cloneUrl } from "../../lib/git-profile-utils";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";
import { useStore } from "../../stores";

const emptyProfile = (): GitProfile => ({ name: "", account: "", alias: "", hostname: "github.com", user: "git", port: 22, key_path: "" });
type Editing = { profile: GitProfile; original: string | null; revision: string };

export function GitProfilesPage() {
  const [snapshot, setSnapshot] = useState<ProfileSnapshot | null>(null);
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [deleting, setDeleting] = useState<{ profile: GitProfile; revision: string } | null>(null);
  const [repository, setRepository] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const showToast = useStore(s => s.showToast);
  const refresh = useCallback(async () => {
    try {
      const [profiles, foundKeys] = await Promise.all([listGitProfiles(), listSshKeys()]);
      setSnapshot(profiles); setKeys(foundKeys.filter(k => !k.error)); setError(null);
    } catch (e) { setError(String(e)); }
  }, []);
  useEffect(() => {
    void refresh();
    const config = subscribe("ssh-config-changed", refresh), keys = subscribe("ssh-keys-changed", refresh);
    return () => { config(); keys(); };
  }, [refresh]);
  function field<K extends keyof GitProfile>(key: K, value: GitProfile[K]) {
    setEditing(current => current ? { ...current, profile: { ...current.profile, [key]: value } } : null);
  }
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!editing || busy) return;
    setBusy(true); setError(null);
    try {
      const result = await saveGitProfile(editing.profile, editing.original, editing.revision);
      setSnapshot(result); setEditing(null); showToast("Git profile saved. Its SSH alias is ready to use.", "success");
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function remove() {
    if (!deleting || busy) return;
    setBusy(true);
    try { setSnapshot(await deleteGitProfile(deleting.profile.alias, deleting.revision)); setDeleting(null); showToast("Profile removed. The key file was kept.", "success"); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function copy(profile: GitProfile) {
    const url = cloneUrl(profile, repository); if (!url) return;
    try { await writeText(url); showToast("SSH clone URL copied", "success"); }
    catch (e) { showToast(`Cannot copy URL: ${e}`, "error"); }
  }
  const input = "input-field mt-1";
  return <section className="space-y-5">
    <header className="flex items-start justify-between gap-3">
      <div><h2 className="text-lg font-semibold">Git Profiles</h2><p className="text-sm text-muted-foreground">Keep work, personal and client Git accounts on their own SSH aliases.</p></div>
      {!editing && <button className="btn btn-primary shrink-0" disabled={!snapshot} onClick={() => setEditing({ profile: emptyProfile(), original: null, revision: snapshot!.revision })}>New profile</button>}
    </header>
    {error && <div role="alert" className="border border-destructive rounded p-3 text-sm"><p>{error}</p><button className="btn underline mt-2" onClick={refresh}>Refresh profiles</button></div>}
    {editing ? <form onSubmit={save} className="max-w-2xl rounded-lg border border-border p-5">
      <h3 className="font-medium mb-4">{editing.original ? "Edit Git profile" : "Create Git profile"}</h3>
      <fieldset disabled={busy} className="grid grid-cols-2 gap-4 text-sm">
        <label>Profile name<input className={input} required value={editing.profile.name} onChange={e => field("name", e.target.value)} placeholder="Work" /></label>
        <label>Account label<input className={input} required value={editing.profile.account} onChange={e => field("account", e.target.value)} placeholder="Your Git account name" /></label>
        <label>SSH alias<input className={input} required pattern="[A-Za-z0-9_.-]+" readOnly={!!editing.original} value={editing.profile.alias} onChange={e => field("alias", e.target.value)} placeholder="github-work" /></label>
        <label>Git service hostname<input className={input} required value={editing.profile.hostname} onChange={e => field("hostname", e.target.value)} placeholder="github.com or git.company.com" /></label>
        <label>SSH user<input className={input} required value={editing.profile.user} onChange={e => field("user", e.target.value)} /></label>
        <label>SSH port<input className={input} type="number" min={1} max={65535} required value={editing.profile.port} onChange={e => field("port", Number(e.target.value))} /></label>
        <label className="col-span-2">SSH key<select className={input} required value={editing.profile.key_path} onChange={e => field("key_path", e.target.value)}>
          <option value="">Choose a key…</option>
          {editing.profile.key_path && !keys.some(k => k.private_path === editing.profile.key_path) && <option value={editing.profile.key_path}>Missing: {editing.profile.key_path}</option>}
          {keys.map(key => <option key={key.private_path} value={key.private_path}>{key.name} · {key.fingerprint}</option>)}
        </select></label>
        <p className="col-span-2 text-xs text-muted-foreground">The account label is for you; Git services usually use the SSH user “git”. Register this key's public part with the selected account. Saving a profile does not register or verify the account.</p>
        {!keys.length && <p className="col-span-2">No usable keys yet. Generate or import one on the SSH Keys page.</p>}
        <div className="col-span-2 flex gap-2"><button type="submit" disabled={!keys.length} className="btn btn-primary">{busy ? "Saving…" : "Save profile"}</button><button type="button" className="btn" onClick={() => { setEditing(null); setError(null); }}>Cancel</button></div>
      </fieldset>
    </form> : <>
      <label className="block text-sm max-w-lg">Repository path<input className={input} value={repository} onChange={e => setRepository(e.target.value)} placeholder="owner/repository or group/subgroup/repository" /></label>
      <p className="text-xs text-muted-foreground">Enter a repository path to get a clone URL for each account. Existing Git remotes are not changed when a profile is saved.</p>
      {snapshot?.profiles.length === 0 && <div className="border border-dashed rounded-lg p-8 text-center"><h3 className="font-medium">Your first Git profile</h3><p className="text-sm text-muted-foreground mt-2">Choose an existing SSH key and give the account its own alias, such as github-work.</p></div>}
      {!snapshot && !error && <p>Loading profiles…</p>}
      <div className="grid gap-3">{snapshot?.profiles.map(profile => <article key={profile.alias} className="rounded-lg border border-border bg-card p-4 space-y-2">
        <div className="flex items-start justify-between gap-3"><div><h3 className="font-semibold">{profile.name}</h3><p className="text-sm text-muted-foreground">{profile.account} · {profile.hostname}</p></div><div className="flex gap-2 text-sm"><button className="btn" onClick={() => setEditing({ profile: { ...profile }, original: profile.alias, revision: snapshot.revision })}>Edit</button><button className="btn" onClick={() => useStore.getState().openDiagnostics(profile.alias)}>Diagnose</button><button className="btn btn-danger" onClick={() => setDeleting({ profile, revision: snapshot.revision })}>Remove</button></div></div>
        <p className="text-sm"><span className="font-mono">{profile.user}@{profile.alias}</span> · port {profile.port} · IdentitiesOnly</p>
        <p className="font-mono text-xs break-all text-muted-foreground">{profile.key_path}</p>
        {!keys.some(k => k.private_path === profile.key_path) && <p role="status" className="text-sm text-destructive">Key is missing or unreadable. Edit this profile to select another key.</p>}
        {cloneUrl(profile, repository) ? <div className="flex items-center gap-2 pt-2"><code className="text-sm break-all flex-1">{cloneUrl(profile, repository)}</code><button className="btn shrink-0" onClick={() => copy(profile)}>Copy clone URL</button></div> : repository && <p className="text-xs text-destructive">Use a repository path with letters, numbers, dots, hyphens, underscores and slashes.</p>}
      </article>)}</div>
    </>}
    {deleting && <ConfirmDialog title="Remove Git profile" description={`Remove '${deleting.profile.name}' and its SSH alias '${deleting.profile.alias}'? Repositories using that alias will stop connecting until their remote is changed. The key file will be kept.`} confirmLabel={busy ? "Removing…" : "Remove profile"} destructive onConfirm={remove} onCancel={() => { if (!busy) setDeleting(null); }} />}
  </section>;
}
