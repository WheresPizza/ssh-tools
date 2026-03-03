import { useEffect, useCallback, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useShallow } from "zustand/react/shallow";
import { listSshKeys, deleteSshKey, getPublicKey, copyKeyToServer, listAgentKeys, addKeyToAgent, removeKeyFromAgent } from "../../lib/tauri";
import type { SshKeyInfo } from "../../lib/tauri";
import { useStore } from "../../stores";
import { KeyList } from "./KeyList";
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

  const [showGenerator, setShowGenerator] = useState(false);
  const [copyToServerKey, setCopyToServerKey] = useState<SshKeyInfo | null>(null);
  const [agentFingerprints, setAgentFingerprints] = useState<string[]>([]);

  const loadAgentKeys = useCallback(async () => {
    try {
      const fps = await listAgentKeys();
      setAgentFingerprints(fps);
    } catch {
      // silently ignore — agent may not be running
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
    loadKeys();
    loadAgentKeys();
  }, [loadKeys, loadAgentKeys]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("ssh-keys-changed", () => loadKeys()).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [loadKeys]);

  const handleDelete = async (keyPath: string, name: string) => {
    try {
      await deleteSshKey(keyPath);
      showToast(`Key '${name}' deleted`, "success");
      loadKeys();
    } catch (e) {
      showToast(`Failed to delete key: ${e}`, "error");
    }
  };

  const handleCopyPublicKey = async (keyPath: string) => {
    try {
      const pubKey = await getPublicKey(keyPath);
      await navigator.clipboard.writeText(pubKey);
      showToast("Public key copied to clipboard", "success");
    } catch (e) {
      showToast(`Failed to copy key: ${e}`, "error");
    }
  };

  const handleAddToAgent = async (key: SshKeyInfo) => {
    try {
      await addKeyToAgent(key.private_path);
      showToast(`Key '${key.name}' added to agent`, "success");
      loadAgentKeys();
    } catch (e) {
      showToast(`Failed to add key to agent: ${e}`, "error");
    }
  };

  const handleRemoveFromAgent = async (key: SshKeyInfo) => {
    try {
      await removeKeyFromAgent(key.private_path);
      showToast(`Key '${key.name}' removed from agent`, "success");
      loadAgentKeys();
    } catch (e) {
      showToast(`Failed to remove key from agent: ${e}`, "error");
    }
  };

  const handleCopyToServerClick = (key: SshKeyInfo) => {
    if (hosts.length === 0) {
      showToast("Add SSH hosts in the SSH Config tab first", "info");
      return;
    }
    setCopyToServerKey(key);
  };

  const handleCopyToServer = async (hostAlias: string) => {
    if (!copyToServerKey) return;
    try {
      await copyKeyToServer(copyToServerKey.private_path, hostAlias);
      showToast(`Opening terminal to copy ${copyToServerKey.name} to ${hostAlias}`, "success");
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
    } finally {
      setCopyToServerKey(null);
    }
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <div>
          <h2 className="text-lg font-semibold">SSH Keys</h2>
          <p className="text-sm text-muted-foreground">~/.ssh/</p>
        </div>
        <button
          onClick={() => setShowGenerator(true)}
          className="px-3 py-1.5 bg-primary text-primary-foreground text-sm rounded-md hover:bg-primary/90 transition-colors font-medium"
        >
          Generate Key
        </button>
      </div>

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
              className="px-3 py-1.5 bg-primary text-primary-foreground text-sm rounded-md hover:bg-primary/90 transition-colors"
            >
              Generate Key
            </button>
          }
        />
      ) : (
        <KeyList
          keys={keys}
          agentFingerprints={agentFingerprints}
          onDelete={handleDelete}
          onCopyPublicKey={handleCopyPublicKey}
          onCopyToServer={handleCopyToServerClick}
          onAddToAgent={handleAddToAgent}
          onRemoveFromAgent={handleRemoveFromAgent}
        />
      )}

      {showGenerator && (
        <KeyGeneratorDialog
          onClose={() => { setShowGenerator(false); loadKeys(); }}
          onCancel={() => setShowGenerator(false)}
        />
      )}

      {copyToServerKey && (
        <CopyToServerDialog
          keyName={copyToServerKey.name}
          hosts={hosts}
          onConfirm={handleCopyToServer}
          onCancel={() => setCopyToServerKey(null)}
        />
      )}
    </div>
  );
}
