import { useStore } from "./stores";
import { AppShell } from "./components/layout/AppShell";
import { SshConfigPage } from "./features/ssh-config/SshConfigPage";
import { SshKeysPage } from "./features/ssh-keys/SshKeysPage";
import { LauncherPage } from "./features/launcher/LauncherPage";
import { KnownHostsPage } from "./features/known-hosts/KnownHostsPage";

function PageContent() {
  const activeTab = useStore((s) => s.activeTab);

  switch (activeTab) {
    case "ssh-config":
      return <SshConfigPage />;
    case "ssh-keys":
      return <SshKeysPage />;
    case "launcher":
      return <LauncherPage />;
    case "known-hosts":
      return <KnownHostsPage />;
    default:
      return <SshConfigPage />;
  }
}

function App() {
  return (
    <AppShell>
      <PageContent />
    </AppShell>
  );
}

export default App;
