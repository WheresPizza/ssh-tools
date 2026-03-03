import { useEffect, useCallback, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useShallow } from "zustand/react/shallow";
import { listKnownHosts, deleteKnownHost, verifyKnownHost } from "../../lib/tauri";
import type { KnownHostEntry } from "../../lib/tauri";
import { useStore } from "../../stores";
import { KnownHostsTable } from "./KnownHostsTable";
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

  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [verifyStatus, setVerifyStatus] = useState<Record<number, 'checking' | 'match' | 'mismatch' | 'error'>>({});

  const loadKnownHosts = useCallback(async () => {
    setKnownHostsLoading(true);
    try {
      const data = await listKnownHosts();
      setKnownHosts(data);
      setSelected(new Set());
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
    let unlisten: (() => void) | undefined;
    listen("known-hosts-changed", () => loadKnownHosts()).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [loadKnownHosts]);

  const handleDelete = async (lineNumber: number) => {
    try {
      await deleteKnownHost(lineNumber);
      showToast("Entry removed", "success");
      loadKnownHosts();
    } catch (e) {
      showToast(`Failed to delete entry: ${e}`, "error");
    }
  };

  const handleVerify = async (entry: KnownHostEntry) => {
    setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: 'checking' }));
    try {
      const match = await verifyKnownHost(entry.hostname, entry.key_type, entry.key_data);
      setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: match ? 'match' : 'mismatch' }));
    } catch {
      setVerifyStatus((prev) => ({ ...prev, [entry.line_number]: 'error' }));
    }
  };

  const handleBulkDelete = async () => {
    const lines = Array.from(selected).sort((a, b) => b - a); // delete from bottom up
    try {
      for (const line of lines) {
        await deleteKnownHost(line);
      }
      showToast(`${lines.length} entries removed`, "success");
      loadKnownHosts();
    } catch (e) {
      showToast(`Bulk delete failed: ${e}`, "error");
    }
  };

  const filtered = knownHosts.filter((h) =>
    h.hostname.toLowerCase().includes(search.toLowerCase()) ||
    h.key_type.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <div>
          <h2 className="text-lg font-semibold">Known Hosts</h2>
          <p className="text-sm text-muted-foreground">~/.ssh/known_hosts</p>
        </div>
        <button
          onClick={loadKnownHosts}
          className="px-3 py-1.5 text-sm rounded-md border border-border hover:bg-accent transition-colors"
        >
          Refresh
        </button>
      </div>

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
            className="px-3 py-1.5 text-sm rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
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
