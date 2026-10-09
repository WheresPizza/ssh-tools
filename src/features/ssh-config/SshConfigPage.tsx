import { useEffect, useState, useCallback, useRef } from "react";
import { subscribe } from "../../lib/events";
import { useShallow } from "zustand/react/shallow";
import { getSshConfig, addHost, updateHost, deleteHost, reorderHosts, launchSshConnection } from "../../lib/tauri";
import type { SshHost } from "../../lib/tauri";
import { filterHosts, mergeVisibleHostOrder } from "../../lib/ssh-host-utils";
import { useStore } from "../../stores";
import { HostList } from "./HostList";
import { HostEditor } from "./HostEditor";
import { EmptyState } from "../../components/common/EmptyState";

export function SshConfigPage() {
  const { hosts, loading, setHosts, setLoading, showToast, recordConnection, connectionHistory } = useStore(
    useShallow((s) => ({
      hosts: s.hosts,
      loading: s.loading,
      setHosts: s.setHosts,
      setLoading: s.setLoading,
      showToast: s.showToast,
      recordConnection: s.recordConnection,
      connectionHistory: s.connectionHistory,
    }))
  );

  const [error, setError] = useState<string | null>(null);
  const lock = useRef(false), request = useRef(0);
  const [busy, setBusy] = useState(false);
  const [editingHost, setEditingHost] = useState<SshHost | null>(null);
  const [isAdding, setIsAdding] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const filteredHosts = filterHosts(hosts, searchQuery);

  const loadHosts = useCallback(async () => {
    const sequence = ++request.current;
    setLoading(true);
    try {
      const data = await getSshConfig();
      if (sequence === request.current) setHosts(data);
    } catch (e) {
      showToast(`Failed to load SSH config: ${e}`, "error");
    } finally {
      if (sequence === request.current) setLoading(false);
    }
  }, [setHosts, setLoading, showToast]);

  useEffect(() => {
    loadHosts();
  }, [loadHosts]);

  useEffect(() => {
    return subscribe("ssh-config-changed", () => loadHosts());
  }, [loadHosts]);

  const handleAdd = () => {
    setError(null);
    setEditingHost(null);
    setIsAdding(true);
  };

  const handleEdit = (host: SshHost) => {
    setError(null);
    setEditingHost(host);
    setIsAdding(false);
  };

  const handleDelete = async (host: SshHost) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    const alias = host.alias;
    try {
      await deleteHost(alias, host.revision ?? "", host.source_path ?? "", host.line_start);
      showToast(`Host '${alias}' deleted`, "success");
      loadHosts();
    } catch (e) {
      setError(`Failed to delete host: ${e}`);
    } finally { lock.current = false; setBusy(false); }
  };

  const handleSave = async (host: SshHost) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try {
      if (isAdding) {
        await addHost(host);
        showToast(`Host '${host.alias}' added`, "success");
      } else if (editingHost) {
        await updateHost(editingHost.alias, host);
        showToast(`Host '${host.alias}' updated`, "success");
      }
      setIsAdding(false);
      setEditingHost(null);
      loadHosts();
    } catch (e) {
      setError(`Failed to save host: ${e}`);
    } finally { lock.current = false; setBusy(false); }
  };

  const handleReorder = async (reordered: SshHost[]) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    const complete = mergeVisibleHostOrder(hosts, reordered);
    setHosts(complete);
    try {
      const sources = new Set(complete.map(h => h.source_path ?? ""));
      for (const source of sources) {
        const group = complete.filter(h => (h.source_path ?? "") === source);
        const previous = hosts.filter(h => (h.source_path ?? "") === source);
        if (group.every((h,i) => h.alias === previous[i]?.alias)) continue;
        await reorderHosts(group.map(h => h.alias), group[0]?.revision ?? "", source);
      }
      await loadHosts();
    } catch (e) {
      setError(`Failed to reorder: ${e}`);
      await loadHosts();
    } finally { lock.current = false; setBusy(false); }
  };

  const handleConnect = async (host: SshHost) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try { await launchSshConnection(host.alias); recordConnection(host.alias); showToast("Terminal launched. Authentication is completed in the terminal; it has not been verified here.", "info"); }
    catch (e) { setError(`Failed to launch: ${e}`); }
    finally { lock.current = false; setBusy(false); }
  };

  const handleDuplicate = (host: SshHost) => {
    setEditingHost({ ...host, source_path: host.read_only ? "" : host.source_path, read_only: false, alias: `${host.alias}-copy` });
    setIsAdding(true);
  };

  const handleCancel = () => {
    setIsAdding(false);
    setEditingHost(null);
  };

  if (isAdding || editingHost) {
    return (
      <HostEditor
        error={error}
        host={editingHost}
        isNew={isAdding}
        onSave={handleSave}
        onCancel={handleCancel}
      />
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <div>
          <h2 className="text-lg font-semibold">SSH Hosts</h2>
          <p className="text-sm text-muted-foreground">Connection aliases and rules from SSH configuration.</p>
        </div>
        <button
          onClick={handleAdd}
          className="btn btn-primary"
        >
          + Add Host
        </button>
      </div>

      {error && <p role="alert" className="mb-4 text-destructive text-sm break-words">{error} <button className="btn btn-compact" onClick={() => setError(null)}>Dismiss error</button></p>}
      <button className="btn btn-compact mb-4" onClick={() => useStore.getState().setActiveTab("settings")}>File recovery in Settings</button>
      <p className="mb-4 text-xs text-muted-foreground">Git profile aliases are generated configuration and remain read-only here. <button className="underline text-primary" onClick={() => useStore.getState().setActiveTab("git-profiles")}>Manage Git Profiles</button></p>
      {loading ? (
        <div className="flex items-center justify-center py-16">
          <div className="text-muted-foreground text-sm">Loading...</div>
        </div>
      ) : hosts.length === 0 ? (
        <EmptyState
          title="No SSH hosts configured"
          description="Add your first SSH host to get started."
          action={
            <button
              onClick={handleAdd}
              className="btn btn-primary"
            >
              Add Host
            </button>
          }
        />
      ) : (
        <>
          <div className="mb-4">
            <input
              type="search"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search by alias, hostname, or user…"
              className="input-field w-full"
            />
          </div>
          {filteredHosts.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-10">
              No results for "{searchQuery}"
            </p>
          ) : (
            <fieldset disabled={busy}><HostList
              hosts={filteredHosts}
              connectionHistory={connectionHistory}
              onEdit={handleEdit}
              onDelete={handleDelete}
              onReorder={handleReorder}
              onConnect={handleConnect}
              onDuplicate={handleDuplicate}
            /></fieldset>
          )}
        </>
      )}
    </div>
  );
}
