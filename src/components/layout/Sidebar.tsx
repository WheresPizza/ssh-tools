import { useStore } from "../../stores";
import { SidebarNavItem } from "./SidebarNavItem";

const NAV_ITEMS = [
  { id: "ssh-config", label: "SSH Config", icon: "⚙" },
  { id: "ssh-keys", label: "SSH Keys", icon: "🔑" },
  { id: "launcher", label: "Launcher", icon: "▶" },
  { id: "known-hosts", label: "Known Hosts", icon: "📋" },
];

export function Sidebar() {
  const activeTab = useStore((s) => s.activeTab);
  const setActiveTab = useStore((s) => s.setActiveTab);

  return (
    <aside className="w-52 shrink-0 border-r border-border bg-muted/30 flex flex-col pt-4 pb-2">
      <div className="px-4 mb-6">
        <h1 className="text-sm font-bold text-foreground tracking-wide">SSH GUI</h1>
        <p className="text-xs text-muted-foreground mt-0.5">Manage your SSH</p>
      </div>
      <nav className="flex flex-col gap-1 px-2">
        {NAV_ITEMS.map((item) => (
          <SidebarNavItem
            key={item.id}
            id={item.id}
            label={item.label}
            icon={item.icon}
            active={activeTab === item.id}
            onClick={() => setActiveTab(item.id)}
          />
        ))}
      </nav>
    </aside>
  );
}
