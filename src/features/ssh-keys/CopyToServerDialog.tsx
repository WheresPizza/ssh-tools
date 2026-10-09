import { useDialog } from "../../components/common/useDialog";
import { useState } from "react";
import type { SshHost } from "../../lib/tauri";

interface CopyToServerDialogProps {
  keyName: string;
  busy?: boolean;
  error?: string | null;
  hosts: SshHost[];
  onConfirm: (hostAlias: string) => void;
  onCancel: () => void;
}

export function CopyToServerDialog({ keyName, hosts, onConfirm, onCancel, busy = false, error = null }: CopyToServerDialogProps) {
  const panel = useDialog(onCancel, busy);
  const [selected, setSelected] = useState<string>(hosts[0]?.alias ?? "");

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div ref={panel} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="copy-title" className="bg-background rounded-lg border border-border p-5 w-80 shadow-xl max-h-[85vh] overflow-auto mx-4">
        <h3 id="copy-title" className="font-semibold mb-1">Copy Key to Server</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Push <span className="font-mono">{keyName}.pub</span> to a server's authorized_keys
        </p>
        <p className="text-xs text-muted-foreground mb-3">This appends the selected public key, even when another identity already grants access. Repeating it can add a duplicate entry. Complete authentication in the terminal.</p>
        <fieldset disabled={busy}>
        <select
          aria-label="Destination host"
          value={selected}
          onChange={(e) => setSelected(e.target.value)}
          className="input-field"
        >
          {hosts.map((h) => (
            <option key={`${h.source_path}:${h.line_start}:${h.alias}`} value={h.alias}>
              {h.alias}{h.hostname ? ` (${h.hostname})` : ""}
            </option>
          ))}
        </select>
        {error && <p role="alert" className="text-sm text-destructive break-words mt-3">{error}</p>}
        <div className="flex gap-2 mt-4 justify-end">
          <button
            onClick={onCancel}
            className="btn"
          >
            Cancel
          </button>
          <button
            onClick={() => onConfirm(selected)}
            disabled={!selected}
            className="btn btn-primary disabled:opacity-50"
          >
            {busy ? "Starting…" : "Copy Key"}
          </button>
        </div>
        </fieldset>
      </div>
    </div>
  );
}
