import { useEffect, useState, useCallback } from "react";
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

  const [editingHost, setEditingHost] = useState<SshHost | null>(null);
  const [isAdding, setIsAdding] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const filteredHosts = filterHosts(hosts, searchQuery);

  const loadHosts = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getSshConfig();
      setHosts(data);
    } catch (e) {
      showToast(`Failed to load SSH config: ${e}`, "error");
    } finally {
      setLoading(false);
    }
  }, [setHosts, setLoading, showToast]);

  useEffect(() => {
    loadHosts();
  }, [loadHosts]);

  useEffect(() => {
    return subscribe("ssh-config-changed", () => loadHosts());
  }, [loadHosts]);

  const handleAdd = () => {
    setEditingHost(null);
    setIsAdding(true);
  };

  const handleEdit = (host: SshHost) => {
    setEditingHost(host);
    setIsAdding(false);
  };

  const handleDelete = async (host: SshHost) => {
    const alias = host.alias;
    try {
      await deleteHost(alias, host.revision ?? "", host.source_path ?? "", host.line_start);
      showToast(`Host '${alias}' deleted`, "success");
      loadHosts();
    } catch (e) {
      showToast(`Failed to delete host: ${e}`, "error");
    }
  };

  const handleSave = async (host: SshHost) => {
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
      showToast(`Failed to save host: ${e}`, "error");
    }
  };

  const handleReorder = async (reordered: SshHost[]) => {
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
      showToast(`Failed to reorder: ${e}`, "error");
      loadHosts();
    }
  };

  const handleConnect = (host: SshHost) => {
    launchSshConnection(host.alias).then(() => recordConnection(host.alias)).catch((e) => showToast(`Failed to launch: ${e}`, "error"));
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
            <HostList
              hosts={filteredHosts}
              connectionHistory={connectionHistory}
              onEdit={handleEdit}
              onDelete={handleDelete}
              onReorder={handleReorder}
              onConnect={handleConnect}
              onDuplicate={handleDuplicate}
            />
          )}
        </>
      )}
    </div>
  );
}
