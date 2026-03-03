import { cn } from "../../lib/utils";

interface SidebarNavItemProps {
  id: string;
  label: string;
  icon: string;
  active: boolean;
  onClick: () => void;
}

export function SidebarNavItem({ label, icon, active, onClick }: SidebarNavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full flex items-center gap-2.5 px-3 py-2 rounded-md text-sm font-medium transition-colors text-left",
        active
          ? "bg-primary text-primary-foreground"
          : "text-muted-foreground hover:text-foreground hover:bg-accent"
      )}
    >
      <span className="text-base leading-none">{icon}</span>
      <span>{label}</span>
    </button>
  );
}
