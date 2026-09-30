import { DiagnosticsPage } from "./features/diagnostics/DiagnosticsPage";
import { RepositoriesPage } from "./features/repositories/RepositoriesPage";
import { GitProfilesPage } from "./features/git-profiles/GitProfilesPage";
import { useEffect, useState } from "react";
import { getWorkspace } from "./lib/tauri";
import { useStore } from "./stores";
import { AppShell } from "./components/layout/AppShell";
import { SshConfigPage } from "./features/ssh-config/SshConfigPage";
import { SshKeysPage } from "./features/ssh-keys/SshKeysPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { KnownHostsPage } from "./features/known-hosts/KnownHostsPage";

function PageContent() {
  const activeTab = useStore((s) => s.activeTab);

  switch (activeTab) {
    case "diagnostics":
      return <DiagnosticsPage />;
    case "repositories":
      return <RepositoriesPage />;
    case "git-profiles":
      return <GitProfilesPage />;
    case "ssh-config":
      return <SshConfigPage />;
    case "ssh-keys":
      return <SshKeysPage />;
    case "settings":
      return <SettingsPage />;
    case "launcher":
      return <SshConfigPage />;
    case "known-hosts":
      return <KnownHostsPage />;
    default:
      return <SshConfigPage />;
  }
}

function App() {
  const [workspace, setWorkspace] = useState<{ ssh_dir: string; isolated: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    getWorkspace().then(async value => {
      useStore.persist.setOptions({ name: value.isolated ? `ssh-gui-storage:${value.ssh_dir}` : "ssh-gui-storage" });
      await useStore.persist.rehydrate();
      if (useStore.getState().activeTab === "launcher") useStore.getState().setActiveTab("ssh-config");
      if (active) setWorkspace(value);
    }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, []);
  if (error) return <main className="p-6"><h1>Cannot open SSH workspace</h1><p role="alert">{error}</p></main>;
  if (!workspace) return <p className="p-6">Opening SSH workspace…</p>;
  return (
    <AppShell>
      {workspace.isolated && <p className="workspace-banner" role="status">Isolated workspace: {workspace.ssh_dir}</p>}
      <PageContent />
    </AppShell>
  );
}

export default App;
