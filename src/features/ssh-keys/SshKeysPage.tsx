import { listKeyMetadata, type MetadataSnapshot } from "../../lib/tauri";
import { KeyDetails } from "./KeyDetails";
import { KeyAuditPanel } from "./KeyAuditPanel";
import { isConnectableAlias } from "../../lib/ssh-host-utils";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AgentOptionsDialog } from "./AgentOptionsDialog";
import { listAgentEnrollments, type AgentOptions, type AgentEnrollment } from "../../lib/tauri";
import { PermissionsAudit } from "./PermissionsAudit";
import { getSshConfig } from "../../lib/tauri";
import { useEffect, useCallback, useState } from "react";
import { subscribe } from "../../lib/events";
import { useShallow } from "zustand/react/shallow";
import { listSshKeys, deleteSshKey, getPublicKey, copyKeyToServer, listAgentKeys, addKeyToAgent, removeKeyFromAgent } from "../../lib/tauri";
import type { SshKeyInfo } from "../../lib/tauri";
import { useStore } from "../../stores";
import { KeyList } from "./KeyList";
import { KeyImportDialog } from "./KeyImportDialog";
import { KeyGeneratorDialog } from "./KeyGeneratorDialog";
import { CopyToServerDialog } from "./CopyToServerDialog";
import { EmptyState } from "../../components/common/EmptyState";

export function SshKeysPage() {
  const { keys, keysLoading, hosts, setKeys, setKeysLoading, showToast } = useStore(
    useShallow((s) => ({
      keys: s.keys,
      keysLoading: s.keysLoading,
      hosts: s.hosts,
      setKeys: s.setKeys,
      setKeysLoading: s.setKeysLoading,
      showToast: s.showToast,
    }))
  );

  const [metadata, setMetadata] = useState<MetadataSnapshot | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [tag, setTag] = useState("");
  const [showAudit, setShowAudit] = useState(false);
  const [dataRevision, setDataRevision] = useState(0);
  const loadMetadata = useCallback(async () => {
    try { setMetadata(await listKeyMetadata()); setMetadataError(null); }
    catch (e) { setMetadataError(`Cannot load annotations: ${e}`); }
  }, []);
  useEffect(() => { void loadMetadata(); window.addEventListener("focus", loadMetadata); return () => window.removeEventListener("focus", loadMetadata); }, [loadMetadata]);
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [showImporter, setShowImporter] = useState(false);
  const [showGenerator, setShowGenerator] = useState(false);
  const [copyToServerKey, setCopyToServerKey] = useState<SshKeyInfo | null>(null);
  const [agentKey, setAgentKey] = useState<SshKeyInfo | null>(null);
  const [enrollments, setEnrollments] = useState<AgentEnrollment[]>([]);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [agentFingerprints, setAgentFingerprints] = useState<string[]>([]);

  const loadAgentKeys = useCallback(async () => {
    try {
      const fps = await listAgentKeys();
      setAgentFingerprints(fps);
      setEnrollments(await listAgentEnrollments());
      setAgentError(null);
    } catch (error) {
      setAgentFingerprints([]);
      setAgentError(String(error));
    }
  }, []);

  const loadKeys = useCallback(async () => {
    setKeysLoading(true);
    try {
      const data = await listSshKeys();
      setKeys(data);
    } catch (e) {
      showToast(`Failed to load keys: ${e}`, "error");
    } finally {
      setKeysLoading(false);
    }
  }, [setKeys, setKeysLoading, showToast]);

  useEffect(() => {
    const loadHosts = () => getSshConfig().then(useStore.getState().setHosts).catch(e => showToast(`Failed to load hosts: ${e}`, "error"));
    void loadHosts();
    return subscribe("ssh-config-changed", loadHosts);
  }, [showToast]);

  useEffect(() => {
    const refresh = () => { void loadAgentKeys(); };
    const interval = window.setInterval(refresh, 15000);
    window.addEventListener("focus", refresh);
    return () => { window.clearInterval(interval); window.removeEventListener("focus", refresh); };
  }, [loadAgentKeys]);

  useEffect(() => {
    loadKeys();
    loadAgentKeys();
  }, [loadKeys, loadAgentKeys]);

  useEffect(() => {
    return subscribe("ssh-keys-changed", () => { setDataRevision(n => n + 1); return loadKeys(); });
  }, [loadKeys]);

  const handleDelete = async (keyPath: string, name: string, fingerprint: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await deleteSshKey(keyPath, fingerprint);
      showToast(`Key '${name}' deleted`, "success");
      loadKeys();
      loadAgentKeys();
    } catch (e) {
      showToast(`Failed to delete key: ${e}`, "error");
    } finally { setBusy(false); }
  };

  const handleCopyPublicKey = async (keyPath: string) => {
    try {
      const pubKey = await getPublicKey(keyPath);
      await writeText(pubKey);
      showToast("Public key copied to clipboard", "success");
    } catch (e) {
      showToast(`Failed to copy key: ${e}`, "error");
    }
  };

  const handleAddToAgent = async (key: SshKeyInfo, options: AgentOptions) => {
    if (busy) return;
    setBusy(true);
    setAgentKey(null);
    try {
      const added = await addKeyToAgent(key.private_path, options);
      showToast(added ? `Key '${key.name}' added to agent` : "Enter the key passphrase in the terminal, then return here", added ? "success" : "info");
      loadAgentKeys();
    } catch (e) {
      showToast(`Failed to add key to agent: ${e}`, "error");
    } finally { setBusy(false); }
  };

  const handleRemoveFromAgent = async (key: SshKeyInfo) => {
    if (busy) return;
    setBusy(true);
    try {
      await removeKeyFromAgent(key.private_path);
      showToast(`Key '${key.name}' removed from agent`, "success");
      loadAgentKeys();
    } catch (e) {
      showToast(`Failed to remove key from agent: ${e}`, "error");
    } finally { setBusy(false); }
  };

  const handleCopyToServerClick = (key: SshKeyInfo) => {
    if (!hosts.some(h => isConnectableAlias(h.alias))) {
      showToast("Add SSH hosts in the SSH Hosts screen first", "info");
      return;
    }
    setCopyToServerKey(key);
  };

  const handleCopyToServer = async (hostAlias: string) => {
    if (!copyToServerKey) return;
    if (busy) return;
    setBusy(true);
    try {
      await copyKeyToServer(copyToServerKey.private_path, hostAlias);
      showToast(`Opening terminal to copy ${copyToServerKey.name} to ${hostAlias}`, "success");
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
    } finally {
      setBusy(false);
      setCopyToServerKey(null);
    }
  };

  const filteredKeys = keys.filter(k => {
    const annotations = metadata?.entries[k.fingerprint];
    return (!tag || annotations?.tags.includes(tag)) && `${k.name} ${k.comment ?? ""} ${k.fingerprint} ${annotations?.tags.join(" ") ?? ""} ${annotations?.purpose ?? ""}`.toLowerCase().includes(search.toLowerCase());
  });
  const tags = [...new Set(keys.flatMap(k => metadata?.entries[k.fingerprint]?.tags ?? []))].sort();
  const selectedKey = keys.find(k => k.private_path === selected);
  if (selected && selectedKey) return <KeyDetails key={`${selectedKey.private_path}:${selectedKey.fingerprint}`} keyInfo={selectedKey}
    copies={keys.filter(k => !k.error && !!selectedKey.fingerprint && k.fingerprint === selectedKey.fingerprint)}
    snapshot={metadata} metadataError={metadataError} onSaved={saved => { setMetadata(saved); showToast("Annotations saved", "success"); }} onReload={loadMetadata}
    agentStatus={agentError ? `Unavailable: ${agentError}` : agentFingerprints.includes(selectedKey.fingerprint) ? "Loaded in agent; current restrictions cannot be queried" : "Not loaded in agent"}
    onClose={() => setSelected(null)} />;
  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-3 mb-5">
        <div>
          <h2 className="text-lg font-semibold">SSH Keys</h2>
          <p className="text-sm text-muted-foreground">Private key files, public keys and SSH agent access.</p>
        </div>
        <div className="flex shrink-0 gap-2"><button className="btn" onClick={() => setShowImporter(true)}>Import Key</button>
        <button
          onClick={() => setShowGenerator(true)}
          className="btn btn-primary"
        >
          Generate Key
        </button></div>
      </div>

      {selected && !selectedKey && <p role="status" className="mb-4 text-warning">The selected key file is no longer in the library.</p>}
      <PermissionsAudit />
      {metadataError && <div role="alert" className="mb-4 text-sm text-destructive">{metadataError} <button className="btn btn-compact" onClick={() => void loadMetadata()}>Retry annotations</button></div>}
      {showAudit && <KeyAuditPanel metadata={metadataError ? null : metadata} revision={dataRevision} onClose={() => setShowAudit(false)} onOpen={setSelected} />}
      {agentError && <p role="status" className="mb-4 rounded border border-border p-3 text-sm">{agentError}</p>}
      <div className="flex flex-wrap gap-2 mb-4">
        <input type="search" aria-label="Search keys" className="input-field w-auto min-w-[240px] flex-1" value={search} onChange={e => setSearch(e.target.value)} placeholder="Search keys, fingerprints, tags or purpose…" />
        <select aria-label="Filter by tag" className="input-field w-auto max-w-[220px]" value={tag} onChange={e => setTag(e.target.value)}><option value="">All tags</option>{tag && !tags.includes(tag) && <option value={tag}>{tag}</option>}{tags.map(t => <option key={t} value={t}>{t}</option>)}</select>
        <button className="btn" onClick={() => setShowAudit(true)} disabled={showAudit}>Audit keys</button>
        <button onClick={() => { void loadKeys(); void loadAgentKeys(); void loadMetadata(); setDataRevision(n => n + 1); }} className="btn">Refresh</button>
      </div>
      <fieldset disabled={busy} className="min-w-0">
      {keysLoading ? (
        <div className="flex items-center justify-center py-16">
          <div className="text-muted-foreground text-sm">Loading...</div>
        </div>
      ) : keys.length === 0 ? (
        <EmptyState
          title="No SSH keys found"
          description="Generate a new key pair to get started."
          action={
            <button
              onClick={() => setShowGenerator(true)}
              className="btn btn-primary"
            >
              Generate Key
            </button>
          }
        />
      ) : filteredKeys.length === 0 ? (
        <EmptyState title="No matching keys" description="Try a different search or tag filter." />
      ) : (
        <KeyList
          keys={filteredKeys}
          metadata={metadata}
          onDetails={key => setSelected(key.private_path)}
          agentFingerprints={agentFingerprints}
          enrollments={enrollments}
          onDelete={handleDelete}
          onCopyPublicKey={handleCopyPublicKey}
          onCopyToServer={handleCopyToServerClick}
          onAddToAgent={setAgentKey}
          onRemoveFromAgent={handleRemoveFromAgent}
        />
      )}

      </fieldset>
      {agentKey && <AgentOptionsDialog keyName={agentKey.name} onConfirm={options => handleAddToAgent(agentKey, options)} onCancel={() => setAgentKey(null)} />}
      {showImporter && <KeyImportDialog onClose={() => setShowImporter(false)} onImported={() => { setShowImporter(false); void loadKeys(); }} />}
      {showGenerator && (
        <KeyGeneratorDialog
          onClose={() => { setShowGenerator(false); loadKeys(); }}
          onCancel={() => setShowGenerator(false)}
        />
      )}

      {copyToServerKey && (
        <CopyToServerDialog
          keyName={copyToServerKey.name}
          hosts={hosts.filter(h => isConnectableAlias(h.alias))}
          onConfirm={handleCopyToServer}
          onCancel={() => setCopyToServerKey(null)}
        />
      )}
    </div>
  );
}
