import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../stores";
import { open } from "@tauri-apps/plugin-dialog";
import { scanRepositories, setRepositoryRoots, type RepositoryScan } from "../../lib/tauri";

export function RepositoriesPage() {
  const [scan, setScan] = useState<RepositoryScan | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const refresh = useCallback(async () => {
    setBusy(true); setError(null);
    try { setScan(await scanRepositories()); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);
  async function changeRoots(roots: string[]) {
    setBusy(true); setError(null);
    try { await setRepositoryRoots(roots); setScan(await scanRepositories()); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function addFolder() {
    try {
      const selected = await open({ directory: true, multiple: true, title: "Choose folders containing Git projects" });
      if (selected) await changeRoots([...new Set([...(scan?.roots ?? []), ...(Array.isArray(selected) ? selected : [selected])])]);
    } catch (e) { setError(String(e)); }
  }
  const found = scan?.remotes.filter(remote => [remote.repository, remote.name, remote.ssh_host, ...remote.profiles, ...remote.key_paths].join(" ").toLowerCase().includes(query.toLowerCase())) ?? [];
  return <section className="space-y-4">
    <header className="flex justify-between gap-3"><div><h2 className="text-lg font-semibold">Repositories & keys</h2><p className="text-sm text-muted-foreground">See the configured path from a local Git remote to its SSH alias and key.</p></div><button disabled={busy} className="btn btn-primary shrink-0" onClick={addFolder}>Add project folder</button></header>
    {error && <p role="alert" className="border border-destructive rounded p-3 text-sm">{error}</p>}
    <div className="rounded border border-border p-3 text-sm space-y-2">
      {scan?.roots.length === 0 && <p>Choose a folder with your projects to discover their Git remotes.</p>}
      {scan?.roots.map(root => <div key={root} className="flex items-center gap-3"><code className="flex-1 break-all">{root}</code><button disabled={busy} className="btn underline shrink-0" onClick={() => changeRoots(scan.roots.filter(p => p !== root))}>Stop scanning</button></div>)}
      <p className="text-xs text-muted-foreground">Scanning is read-only. Git remotes and repository commands are never changed or executed. Hidden and build folders are skipped; limits are reported below.</p>
    </div>
    <div className="flex gap-2"><input type="search" aria-label="Search repositories and keys" className="input-field" value={query} onChange={e => setQuery(e.target.value)} placeholder="Search repositories, profiles, aliases or key paths…" /><button disabled={busy} className="btn" onClick={refresh}>{busy ? "Scanning…" : "Rescan"}</button></div>
    {scan && <p className="text-sm text-muted-foreground">{scan.repositories} repositories · {scan.remotes.length} remote URLs</p>}
    <p className="text-xs text-muted-foreground">These are configured key candidates, not proof of a successful login. SSH Match rules, agent keys, environment overrides and global Git URL rewrites can change the effective identity.</p>
    {scan?.warnings.length ? <details className="border rounded p-3 text-sm"><summary>Scan warnings ({scan.warnings.length})</summary>{scan.warnings.map((warning,i) => <p key={i} className="mt-2 break-all">{warning}</p>)}</details> : null}
    {!busy && scan && found.length === 0 && <p className="p-6 text-center text-muted-foreground">{query ? "No matching repositories" : "No remote URLs found in the selected folders"}</p>}
    <div className="grid gap-3">{found.map((remote,i) => <article key={`${remote.repository}:${remote.name}:${remote.direction}:${i}`} className="rounded-lg border border-border p-4 space-y-2">
      <div className="flex justify-between gap-3"><h3 className="font-medium break-all">{remote.repository.split("/").pop()}</h3><span className="text-xs text-muted-foreground">{remote.name} · {remote.direction}</span></div>
      <p className="text-xs text-muted-foreground break-all">{remote.repository}</p><code className="block text-xs break-all">{remote.url}</code>
      {remote.ssh_host ? <div className="rounded bg-muted p-3 text-sm space-y-2"><p>SSH alias / host: <strong className="font-mono">{remote.ssh_host}</strong>{remote.profiles.length > 0 && <> · {remote.profiles.join(", ")}</>}</p>
        <button className="btn btn-compact underline" onClick={() => useStore.getState().openDiagnostics(`${remote.ssh_user ? `${remote.ssh_user}@` : ""}${remote.ssh_host!}`, remote.ssh_port)}>Diagnose this host</button>
        {remote.key_paths.length ? remote.key_paths.map(path => <p key={path} className="font-mono text-xs break-all">↳ {path}</p>) : <p className="text-muted-foreground">No explicit key mapping found. Default keys or an SSH agent may be used.</p>}
      </div> : <p className="text-xs text-muted-foreground">This remote does not directly use SSH.</p>}
      {remote.warnings.map((warning,j) => <p key={j} role="status" className="text-xs text-warning">{warning}</p>)}
    </article>)}</div>
  </section>;
}
