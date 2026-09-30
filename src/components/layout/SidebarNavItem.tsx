import type { ReactNode } from "react";

interface SidebarNavItemProps {
  id: string;
  label: string;
  icon: ReactNode;
  active: boolean;
  onClick: () => void;
}

export function SidebarNavItem({ label, icon, active, onClick }: SidebarNavItemProps) {
  return (
    <button onClick={onClick} aria-current={active ? "page" : undefined} className="nav-item">
      <span className="shrink-0">{icon}</span><span>{label}</span>
    </button>
  );
}
