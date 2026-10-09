import { useEffect, useCallback, useState, useRef } from "react";
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

  const [error, setError] = useState<string | null>(null);
  const [verifyErrors, setVerifyErrors] = useState<Record<number, string>>({});
  const generation = useRef(0);
  const verifying = useRef(new Set<number>());
  const [pendingDelete, setPendingDelete] = useState<KnownHostEntry[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [verifyStatus, setVerifyStatus] = useState<Record<number, 'checking' | 'match' | 'mismatch' | 'error'>>({});

  const loadKnownHosts = useCallback(async () => {
    const request = ++generation.current;
    setKnownHostsLoading(true);
    try {
      const data = await listKnownHosts();
      if (request !== generation.current) return;
      setVerifyErrors({});
      setKnownHosts(data);
      setSelected(new Set());
      setVerifyStatus({});
    } catch (e) {
      showToast(`Failed to load known hosts: ${e}`, "error");
    } finally {
      if (request === generation.current) setKnownHostsLoading(false);
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
    if (entry) { setError(null); setPendingDelete([entry]); }
  };

  const confirmDelete = async () => {
    if (!pendingDelete || deleting) return;
    setError(null); setDeleting(true);
    try {
      await deleteKnownHosts(pendingDelete);
      showToast(`${pendingDelete.length} entries removed`, "success");
      setPendingDelete(null);
      await loadKnownHosts();
    } catch (e) { setError(`Failed to remove entries: ${e}`); }
    finally { setDeleting(false); }
  };

  const handleVerify = async (entry: KnownHostEntry) => {
    if (verifying.current.has(entry.line_number)) return;
    verifying.current.add(entry.line_number);
    const request = generation.current;
    const current = () => request === generation.current && useStore.getState().knownHosts.some(h => h.line_number === entry.line_number && h.hostname === entry.hostname && h.key_data === entry.key_data && h.key_type === entry.key_type);
    setVerifyErrors(prev => { const next = { ...prev }; delete next[entry.line_number]; return next; });
    setVerifyStatus(prev => ({ ...prev, [entry.line_number]: 'checking' }));
    try {
      const match = await verifyKnownHost(entry.hostname, entry.key_type, entry.key_data);
      if (current()) setVerifyStatus(prev => ({ ...prev, [entry.line_number]: match ? 'match' : 'mismatch' }));
    } catch (e) {
      if (current()) { setVerifyStatus(prev => ({ ...prev, [entry.line_number]: 'error' })); setVerifyErrors(prev => ({ ...prev, [entry.line_number]: String(e) })); }
    } finally { verifying.current.delete(entry.line_number); }
  };

  const handleBulkDelete = () => setPendingDelete(knownHosts.filter(e => selected.has(e.line_number)));

  const filtered = knownHosts.filter((h) =>
    h.hostname.toLowerCase().includes(search.toLowerCase()) ||
    h.key_type.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div>
      {pendingDelete && <ConfirmDialog busy={deleting} error={error} title="Remove trusted host entries" description={`Remove ${pendingDelete.length} entries? SSH will ask you to trust these hosts again on the next connection.`} onConfirm={confirmDelete} onCancel={() => { if (!deleting) setPendingDelete(null); }} confirmLabel={deleting ? "Removing…" : "Remove"} destructive />}
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
          verifyErrors={verifyErrors}
          onVerify={handleVerify}
        />
      )}
    </div>
  );
}
