import { useEffect, useCallback, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { getDetectedTerminal, setPreferredTerminal, launchSshConnection } from "../../lib/tauri";
import { getSshConfig } from "../../lib/tauri";
import { useStore } from "../../stores";
import type { SshHost } from "../../lib/tauri";
import { EmptyState } from "../../components/common/EmptyState";

export function LauncherPage() {
  const { terminals, setTerminals, hosts, setHosts, showToast } = useStore(
    useShallow((s) => ({
      terminals: s.terminals,
      setTerminals: s.setTerminals,
      hosts: s.hosts,
      setHosts: s.setHosts,
      showToast: s.showToast,
    }))
  );

  const [loading, setLoading] = useState(false);
  const [launching, setLaunching] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const [terms, hostsData] = await Promise.all([
        getDetectedTerminal(),
        getSshConfig(),
      ]);
      setTerminals(terms);
      setHosts(hostsData);
    } catch (e) {
      showToast(`Failed to load: ${e}`, "error");
    } finally {
      setLoading(false);
    }
  }, [setHosts, setTerminals, showToast]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleLaunch = async (alias: string) => {
    setLaunching(alias);
    try {
      await launchSshConnection(alias);
      showToast(`Connecting to ${alias}...`, "info");
    } catch (e) {
      showToast(`Failed to launch: ${e}`, "error");
    } finally {
      setLaunching(null);
    }
  };

  const handleSetTerminal = async (path: string) => {
    try {
      await setPreferredTerminal(path);
      const updated = await getDetectedTerminal();
      setTerminals(updated);
      showToast("Terminal preference saved", "success");
    } catch (e) {
      showToast(`Failed to set terminal: ${e}`, "error");
    }
  };

  const preferredTerminal = terminals.find((t) => t.is_preferred) ?? terminals[0];

  return (
    <div>
      <div className="mb-6">
        <h2 className="text-lg font-semibold mb-1">Connection Launcher</h2>
        <p className="text-sm text-muted-foreground">One-click SSH connections via your system terminal</p>
      </div>

      {/* Terminal selector */}
      <div className="mb-6 p-4 rounded-lg border border-border bg-card">
        <p className="text-sm font-medium mb-2">Terminal</p>
        <div className="flex gap-2 flex-wrap">
          {terminals.map((t) => (
            <button
              key={t.path}
              onClick={() => handleSetTerminal(t.name)}
              className={`px-3 py-1.5 text-sm rounded-md border transition-colors ${
                t.is_preferred
                  ? "border-primary bg-primary/10 text-primary font-medium"
                  : "border-border hover:bg-accent"
              }`}
            >
              {t.name}
            </button>
          ))}
          {terminals.length === 0 && (
            <span className="text-sm text-muted-foreground">No terminals detected</span>
          )}
        </div>
        {preferredTerminal && (
          <p className="text-xs text-muted-foreground mt-2 font-mono">{preferredTerminal.path}</p>
        )}
      </div>

      {/* Host list */}
      {loading ? (
        <div className="flex items-center justify-center py-16">
          <div className="text-muted-foreground text-sm">Loading...</div>
        </div>
      ) : hosts.length === 0 ? (
        <EmptyState
          title="No SSH hosts configured"
          description="Add hosts in the SSH Config tab first."
        />
      ) : (
        <div className="flex flex-col gap-2">
          {hosts.map((host) => (
            <LaunchableHostCard
              key={host.alias}
              host={host}
              launching={launching === host.alias}
              onLaunch={() => handleLaunch(host.alias)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function LaunchableHostCard({
  host,
  launching,
  onLaunch,
}: {
  host: SshHost;
  launching: boolean;
  onLaunch: () => void;
}) {
  return (
    <div className="flex items-center gap-3 p-3 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="font-mono text-sm font-semibold text-foreground">{host.alias}</div>
        <div className="text-xs text-muted-foreground mt-0.5">
          {[host.user && `${host.user}@`, host.hostname || "–", host.port && `:${host.port}`]
            .filter(Boolean)
            .join("")}
        </div>
      </div>
      <button
        onClick={onLaunch}
        disabled={launching}
        className="px-3 py-1.5 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 font-medium shrink-0"
      >
        {launching ? "Launching..." : "Connect"}
      </button>
    </div>
  );
}
