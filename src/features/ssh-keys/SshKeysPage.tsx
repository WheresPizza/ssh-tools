import { useRef } from "react";
import { useOperations } from "../../lib/operations";
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

  const metadataRequest = useRef(0), keyRequest = useRef(0), agentRequest = useRef(0);
  const [metadata, setMetadata] = useState<MetadataSnapshot | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [tag, setTag] = useState("");
  const [showAudit, setShowAudit] = useState(false);
  const [dataRevision, setDataRevision] = useState(0);
  const loadMetadata = useCallback(async () => {
    const request = ++metadataRequest.current;
    try { const value = await listKeyMetadata(); if (request === metadataRequest.current) { setMetadata(value); setMetadataError(null); } }
    catch (e) { if (request === metadataRequest.current) setMetadataError(`Cannot load annotations: ${e}`); }
  }, []);
  useEffect(() => { void loadMetadata(); window.addEventListener("focus", loadMetadata); return () => window.removeEventListener("focus", loadMetadata); }, [loadMetadata]);
  const [search, setSearch] = useState("");
  const actionLock = useRef(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [showImporter, setShowImporter] = useState(false);
  const [showGenerator, setShowGenerator] = useState(false);
  const [copyToServerKey, setCopyToServerKey] = useState<SshKeyInfo | null>(null);
  const [agentKey, setAgentKey] = useState<SshKeyInfo | null>(null);
  const [enrollments, setEnrollments] = useState<AgentEnrollment[]>([]);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [agentFingerprints, setAgentFingerprints] = useState<string[]>([]);

  const loadAgentKeys = useCallback(async () => {
    const request = ++agentRequest.current;
    try {
      const [fps, policies] = await Promise.all([listAgentKeys(), listAgentEnrollments()]);
      if (request !== agentRequest.current) return;
      setAgentFingerprints(fps);
      setEnrollments(policies);
      setAgentError(null);
    } catch (error) {
      if (request !== agentRequest.current) return;
      setAgentFingerprints([]);
      setAgentError(String(error));
    }
  }, []);

  const loadKeys = useCallback(async () => {
    const request = ++keyRequest.current;
    setKeysLoading(true);
    try {
      const data = await listSshKeys();
      if (request === keyRequest.current) setKeys(data);
    } catch (e) {
      showToast(`Failed to load keys: ${e}`, "error");
    } finally {
      if (request === keyRequest.current) setKeysLoading(false);
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
    window.addEventListener("ssh-agent-changed", refresh);
    return () => { window.clearInterval(interval); window.removeEventListener("focus", refresh); window.removeEventListener("ssh-agent-changed", refresh); };
  }, [loadAgentKeys]);

  useEffect(() => {
    loadKeys();
    loadAgentKeys();
  }, [loadKeys, loadAgentKeys]);

  useEffect(() => {
    return subscribe("ssh-keys-changed", () => { setDataRevision(n => n + 1); return loadKeys(); });
  }, [loadKeys]);

  const handleDelete = async (keyPath: string, name: string, fingerprint: string) => {
    if (actionLock.current) return;
    actionLock.current = true;
    setActionError(null);
    setBusy(true);
    try {
      await deleteSshKey(keyPath, fingerprint);
      showToast(`Key '${name}' deleted`, "success");
      loadKeys();
      loadAgentKeys();
    } catch (e) {
      setActionError(`Failed to delete key: ${e}`);
    } finally { actionLock.current = false; setBusy(false); }
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
    if (actionLock.current) return;
    actionLock.current = true;
    setActionError(null);
    setBusy(true);
    try {
      const operation = await addKeyToAgent(key.private_path, options);
      useOperations.getState().track(operation);
      window.dispatchEvent(new Event("terminal-operation-started"));
      setAgentKey(null);
      showToast(operation.state === "completed" ? `Key '${key.name}' added to agent` : "Enter the key passphrase in the terminal; the result will be verified here", operation.state === "completed" ? "success" : "info");
      await loadAgentKeys();
    } catch (e) {
      setActionError(`Failed to add key to agent: ${e}`);
    } finally { actionLock.current = false; setBusy(false); }
  };

  const handleRemoveFromAgent = async (key: SshKeyInfo) => {
    if (actionLock.current) return;
    actionLock.current = true;
    setActionError(null);
    setBusy(true);
    try {
      await removeKeyFromAgent(key.private_path);
      showToast(`Key '${key.name}' removed from agent`, "success");
      loadAgentKeys();
    } catch (e) {
      setActionError(`Failed to remove key from agent: ${e}`);
    } finally { actionLock.current = false; setBusy(false); }
  };

  const handleCopyToServerClick = (key: SshKeyInfo) => {
    if (!hosts.some(h => isConnectableAlias(h.alias))) {
      showToast("Add SSH hosts in the SSH Hosts screen first", "info");
      return;
    }
    setActionError(null);
    setCopyToServerKey(key);
  };

  const handleCopyToServer = async (hostAlias: string) => {
    if (!copyToServerKey) return;
    if (actionLock.current) return;
    actionLock.current = true;
    setActionError(null);
    setBusy(true);
    try {
      const operation = await copyKeyToServer(copyToServerKey.private_path, hostAlias);
      useOperations.getState().track(operation);
      window.dispatchEvent(new Event("terminal-operation-started"));
      setCopyToServerKey(null);
      showToast("Complete authentication in the terminal. Installation is awaiting verification.", "info");
    } catch (e) {
      setActionError(`Failed: ${e}`);
    } finally {
      actionLock.current = false;
      setBusy(false);
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
      {actionError && !agentKey && !copyToServerKey && <div role="alert" className="mb-4 text-sm text-destructive">{actionError} <button className="btn btn-compact" onClick={() => setActionError(null)}>Dismiss error</button></div>}
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
          agentAvailable={!agentError}
          enrollments={enrollments}
          onDelete={handleDelete}
          onCopyPublicKey={handleCopyPublicKey}
          onCopyToServer={handleCopyToServerClick}
          onAddToAgent={key => { setActionError(null); setAgentKey(key); }}
          onRemoveFromAgent={handleRemoveFromAgent}
        />
      )}

      </fieldset>
      {agentKey && <AgentOptionsDialog busy={busy} actionError={actionError} keyName={agentKey.name} onConfirm={options => handleAddToAgent(agentKey, options)} onCancel={() => { if (!busy) { setAgentKey(null); setActionError(null); } }} />}
      {showImporter && <KeyImportDialog onClose={() => setShowImporter(false)} onImported={() => { setShowImporter(false); void loadKeys(); }} />}
      {showGenerator && (
        <KeyGeneratorDialog
          onClose={() => { setShowGenerator(false); loadKeys(); }}
          onCancel={() => setShowGenerator(false)}
        />
      )}

      {copyToServerKey && (
        <CopyToServerDialog
          busy={busy}
          error={actionError}
          keyName={copyToServerKey.name}
          hosts={hosts.filter(h => isConnectableAlias(h.alias))}
          onConfirm={handleCopyToServer}
          onCancel={() => { if (!busy) { setCopyToServerKey(null); setActionError(null); } }}
        />
      )}
    </div>
  );
}
