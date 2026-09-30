import { FolderGit2, GitBranch, FileCog, KeyRound, Activity, Terminal, ShieldCheck, Settings2 } from "lucide-react";
import { useStore } from "../../stores";
import { SidebarNavItem } from "./SidebarNavItem";

const NAV_GROUPS = [
  { label: "WORKSPACE", items: [
    { id: "repositories", label: "Repositories", icon: FolderGit2 },
    { id: "git-profiles", label: "Git Profiles", icon: GitBranch },
    { id: "ssh-config", label: "SSH Hosts", icon: FileCog },
  ] },
  { label: "SECURITY", items: [
    { id: "ssh-keys", label: "SSH Keys", icon: KeyRound },
    { id: "known-hosts", label: "Trusted Hosts", icon: ShieldCheck },
  ] },
  { label: "TOOLS", items: [
    { id: "diagnostics", label: "Diagnostics", icon: Activity },
    { id: "settings", label: "Settings", icon: Settings2 },
  ] },
];

export function Sidebar() {
  const activeTab = useStore((s) => s.activeTab);
  const setActiveTab = useStore((s) => s.setActiveTab);
  return (
    <aside className="w-48 shrink-0 border-r border-border bg-card flex flex-col py-5">
      <div className="px-4 mb-7">
        <h1 className="flex items-center gap-2 text-sm font-semibold"><Terminal size={16} className="text-primary" aria-hidden="true" />SSH GUI</h1>
        <p className="text-xs text-muted-foreground mt-1">SSH workspace</p>
      </div>
      <nav aria-label="Workspace" className="space-y-5 overflow-y-auto pb-4">
        {NAV_GROUPS.map(group => <div key={group.label}>
          <p className="px-4 mb-2 text-xs tracking-widest text-muted-foreground">{group.label}</p>
          <div className="flex flex-col gap-1 px-2">
            {group.items.map(({ icon: Icon, ...item }) => (
              <SidebarNavItem key={item.id} {...item} icon={<Icon size={15} strokeWidth={1.6} aria-hidden="true" />} active={activeTab === item.id} onClick={() => setActiveTab(item.id)} />
            ))}
          </div>
        </div>)}
      </nav>
      <div className="mt-auto mx-4 pt-4 border-t border-border text-xs text-muted-foreground">SSH GUI / v0.3.0</div>
    </aside>
  );
}
