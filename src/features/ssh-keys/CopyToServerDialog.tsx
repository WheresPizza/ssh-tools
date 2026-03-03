import { useState } from "react";
import type { SshHost } from "../../lib/tauri";

interface CopyToServerDialogProps {
  keyName: string;
  hosts: SshHost[];
  onConfirm: (hostAlias: string) => void;
  onCancel: () => void;
}

export function CopyToServerDialog({ keyName, hosts, onConfirm, onCancel }: CopyToServerDialogProps) {
  const [selected, setSelected] = useState<string>(hosts[0]?.alias ?? "");

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-background rounded-lg border border-border p-5 w-80 shadow-xl">
        <h3 className="font-semibold mb-1">Copy Key to Server</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Push <span className="font-mono">{keyName}.pub</span> to a server's authorized_keys
        </p>
        <select
          value={selected}
          onChange={(e) => setSelected(e.target.value)}
          className="w-full px-2 py-1.5 text-sm rounded border border-border bg-background text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
        >
          {hosts.map((h) => (
            <option key={h.alias} value={h.alias}>
              {h.alias}{h.hostname ? ` (${h.hostname})` : ""}
            </option>
          ))}
        </select>
        <div className="flex gap-2 mt-4 justify-end">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm rounded border border-border hover:bg-accent transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => onConfirm(selected)}
            disabled={!selected}
            className="px-3 py-1.5 text-sm rounded bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
          >
            Copy Key
          </button>
        </div>
      </div>
    </div>
  );
}
