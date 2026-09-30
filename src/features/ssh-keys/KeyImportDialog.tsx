import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { inspectKeyImport, importSshKey, type ImportInspection } from "../../lib/tauri";
import { useStore } from "../../stores";
export function KeyImportDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const [source, setSource] = useState("");
  const [filename, setFilename] = useState("");
  const [inspection, setInspection] = useState<ImportInspection | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function choose() {
    setError(null);
    try {
      const selected = await open({ multiple: false, directory: false, title: "Choose an existing private SSH key" });
      if (typeof selected !== "string") return;
      setBusy(true); setInspection(null); setSource(selected); setFilename(selected.split("/").pop() ?? "imported_key");
      setInspection(await inspectKeyImport(selected));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault(); if (!inspection || busy || inspection.duplicates.length) return;
    setBusy(true); setError(null);
    try { const key = await importSshKey(source, filename, inspection.revision); useStore.getState().showToast(`Key '${key.name}' imported`, "success"); onImported(); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center"><section role="dialog" aria-modal="true" aria-labelledby="import-title" className="bg-card border rounded-lg p-6 max-w-lg w-full mx-4 max-h-[85vh] overflow-auto space-y-4">
    <h3 id="import-title" className="font-semibold">Import SSH key</h3><p className="text-sm text-muted-foreground">Copy an existing private key into your SSH library. The source stays unchanged and its passphrase protection is preserved.</p>
    <button disabled={busy} className="btn" onClick={choose}>{source ? "Choose another file" : "Choose private key file"}</button>
    {source && <p className="text-xs font-mono break-all">{source}</p>}
    {busy && <p role="status">Checking or importing key…</p>}
    {error && <p role="alert" className="text-sm text-destructive break-words">{error}</p>}
    {inspection && <form onSubmit={submit} className="space-y-4"><div className="bg-muted rounded p-3 text-sm space-y-2"><p>{inspection.key.algorithm} · {inspection.key.bits ?? "?"} bits · {inspection.key.has_passphrase ? "Encrypted" : "No passphrase"}</p><p className="font-mono text-xs break-all">{inspection.key.fingerprint}</p>{inspection.key.comment && <p>{inspection.key.comment}</p>}{inspection.public_recovered && <p>The missing public companion will be recovered during import.</p>}{inspection.warning && <p role="status" className="text-warning">{inspection.warning}</p>}</div>
      {inspection.duplicates.length ? <div role="status" className="text-sm"><p>This key is already in your library. Use the existing copy:</p>{inspection.duplicates.map(path => <p className="font-mono text-xs break-all mt-1" key={path}>{path}</p>)}</div> : <label className="block text-sm">Library filename<input className="input-field mt-1" required disabled={busy} value={filename} onChange={e => setFilename(e.target.value)} /></label>}
      <button disabled={busy || !!inspection.duplicates.length} className="btn btn-primary disabled:opacity-50" type="submit">Import key</button>
    </form>}
    <button disabled={busy} className="btn" onClick={onClose}>Cancel</button>
  </section></div>;
}
