import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useShallow } from "zustand/react/shallow";
import { getSshConfig, addHost, updateHost, deleteHost, reorderHosts, launchSshConnection } from "../../lib/tauri";
import type { SshHost } from "../../lib/tauri";
import { filterHosts } from "../../lib/ssh-host-utils";
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
    let unlisten: (() => void) | undefined;
    listen("ssh-config-changed", () => loadHosts()).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [loadHosts]);

  const handleAdd = () => {
    setEditingHost(null);
    setIsAdding(true);
  };

  const handleEdit = (host: SshHost) => {
    setEditingHost(host);
    setIsAdding(false);
  };

  const handleDelete = async (alias: string) => {
    try {
      await deleteHost(alias);
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
    setHosts(reordered);
    try {
      await reorderHosts(reordered.map((h) => h.alias));
    } catch (e) {
      showToast(`Failed to reorder: ${e}`, "error");
      loadHosts();
    }
  };

  const handleConnect = (host: SshHost) => {
    recordConnection(host.alias);
    launchSshConnection(host.alias).catch((e) => showToast(`Failed to launch: ${e}`, "error"));
  };

  const handleDuplicate = (host: SshHost) => {
    setEditingHost({ ...host, alias: `${host.alias}-copy` });
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
          <h2 className="text-lg font-semibold">SSH Config</h2>
          <p className="text-sm text-muted-foreground">~/.ssh/config</p>
        </div>
        <button
          onClick={handleAdd}
          className="px-3 py-1.5 bg-primary text-primary-foreground text-sm rounded-md hover:bg-primary/90 transition-colors font-medium"
        >
          + Add Host
        </button>
      </div>

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
              className="px-3 py-1.5 bg-primary text-primary-foreground text-sm rounded-md hover:bg-primary/90 transition-colors"
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
