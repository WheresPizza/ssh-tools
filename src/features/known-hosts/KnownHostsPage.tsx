import { useEffect, useCallback, useState } from "react";
import { subscribe } from "../../lib/events";
import { useShallow } from "zustand/react/shallow";
import { listKnownHosts, deleteKnownHosts, verifyKnownHost } from "../../lib/tauri";
import type { KnownHostEntry } from "../../lib/tauri";
import { useStore } from "../../stores";
import { KnownHostsTable } from "./KnownHostsTable";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";
import { EmptyState } from "../../components/common/EmptyState";

export function KnownHostsPage() {
  const { knownHosts, knownHostsLoading, setKnownHosts, setKnownHostsLoading, showToast } = useStore(
    useShallow((s) => ({
      knownHosts: s.knownHosts,
      knownHostsLoading: s.knownHostsLoading,
      setKnownHosts: s.setKnownHosts,
      setKnownHostsLoading: s.setKnownHostsLoading,
      showToast: s.showToast,
    }))
  );

  const [pendingDelete, setPendingDelete] = useState<KnownHostEntry[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [verifyStatus, setVerifyStatus] = useState<Record<number, 'checking' | 'match' | 'mismatch' | 'error'>>({});

  const loadKnownHosts = useCallback(async () => {
    setKnownHostsLoading(true);
    try {
      const data = await listKnownHosts();
      setKnownHosts(data);
      setSelected(new Set());
      setVerifyStatus({});
    } catch (e) {
      showToast(`Failed to load known hosts: ${e}`, "error");
    } finally {
      setKnownHostsLoading(false);
    }
  }, [setKnownHosts, setKnownHostsLoading, showToast]);

  useEffect(() => {
    loadKnownHosts();
  }, [loadKnownHosts]);

  useEffect(() => {
    return subscribe("known-hosts-changed", () => loadKnownHosts());
  }, [loadKnownHosts]);

  const handleDelete = (lineNumber: number) => {
    const entry = knownHosts.find(e => e.line_number === lineNumber);
    if (entry) setPendingDelete([entry]);
  };

  const confirmDelete = async () => {
    if (!pendingDelete || deleting) return;
    setDeleting(true);
    try {
      await deleteKnownHosts(pendingDelete);
      showToast(`${pendingDelete.length} entries removed`, "success");
      setPendingDelete(null);
      await loadKnownHosts();
    } catch (e) { showToast(`Failed to remove entries: ${e}`, "error"); }
    finally { setDeleting(false); }
  };

  const handleVerify = async (entry: KnownHostEntry) => {
    setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: 'checking' }));
    try {
      const match = await verifyKnownHost(entry.hostname, entry.key_type, entry.key_data);
      if (useStore.getState().knownHosts.some(current => current.line_number === entry.line_number && current.hostname === entry.hostname && current.key_data === entry.key_data)) {
        setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: match ? 'match' : 'mismatch' }));
      }
    } catch {
      setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: 'error' }));
    }
  };

  const handleBulkDelete = () => setPendingDelete(knownHosts.filter(e => selected.has(e.line_number)));

  const filtered = knownHosts.filter((h) =>
    h.hostname.toLowerCase().includes(search.toLowerCase()) ||
    h.key_type.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div>
      {pendingDelete && <ConfirmDialog title="Remove trusted host entries" description={`Remove ${pendingDelete.length} entries? SSH will ask you to trust these hosts again on the next connection.`} onConfirm={confirmDelete} onCancel={() => { if (!deleting) setPendingDelete(null); }} confirmLabel={deleting ? "Removing…" : "Remove"} destructive />}
      <div className="flex items-center justify-between mb-5">
        <div>
          <h2 className="text-lg font-semibold">Trusted Hosts</h2>
          <p className="text-sm text-muted-foreground">Server identities recorded in known_hosts, separate from your own SSH keys.</p>
        </div>
        <button
          onClick={loadKnownHosts}
          className="btn"
        >
          Refresh
        </button>
      </div>

      <button className="btn btn-compact mb-4" onClick={() => useStore.getState().setActiveTab("settings")}>File recovery in Settings</button>
      <p className="mb-4 text-xs text-muted-foreground">Compare key checks the server identity over the network; it does not test your login. Removing an entry removes local trust, not the connection alias.</p>
      <div className="mb-4 flex gap-2">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search by hostname or key type..."
          className="input-field flex-1 max-w-sm"
        />
        {selected.size > 0 && (
          <button
            onClick={handleBulkDelete}
            className="btn btn-danger"
          >
            Delete {selected.size} selected
          </button>
        )}
      </div>

      {knownHostsLoading ? (
        <div className="flex items-center justify-center py-16">
          <div className="text-muted-foreground text-sm">Loading...</div>
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          title={search ? "No matches found" : "No known hosts"}
          description={search ? `No entries match "${search}"` : "No entries in ~/.ssh/known_hosts"}
        />
      ) : (
        <KnownHostsTable
          entries={filtered}
          selected={selected}
          onSelect={setSelected}
          onDelete={handleDelete}
          verifyStatus={verifyStatus}
          onVerify={handleVerify}
        />
      )}
    </div>
  );
}
