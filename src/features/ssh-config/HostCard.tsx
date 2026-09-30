import { isConnectableAlias } from "../../lib/ssh-host-utils";
import { useState } from "react";
import { GripVertical } from "lucide-react";
import type { SshHost } from "../../lib/tauri";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";
import { formatTimeAgo } from "../../lib/time";

interface HostCardProps {
  host: SshHost;
  lastConnected?: number;
  onEdit: () => void;
  onDelete: () => void;
  onConnect: () => void;
  onDuplicate: () => void;
  dragHandleProps?: React.HTMLAttributes<HTMLDivElement>;
}

export function HostCard({ host, lastConnected, onEdit, onDelete, onConnect, onDuplicate, dragHandleProps }: HostCardProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <>
      <div className="flex items-center gap-3 p-3 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors group">
        <div aria-label="Reorder host" {...dragHandleProps} className="cursor-grab text-muted-foreground/40 hover:text-muted-foreground shrink-0 touch-none">
          <GripVertical size={14} />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-mono text-sm font-semibold text-foreground truncate">{host.alias}</span>
            {host.proxy_jump && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">via proxy</span>
            )}
          </div>
          <div className="text-xs text-muted-foreground mt-0.5 truncate">
            {[
              host.user && `${host.user}@`,
              host.hostname || "–",
              host.port && `:${host.port}`,
            ]
              .filter(Boolean)
              .join("")}
          </div>
          {host.source_path && <div className="text-xs text-muted-foreground truncate" title={host.source_path}>{host.source_path}{host.read_only ? " · read-only" : ""}</div>}
          {host.identity_file.length > 0 && (
            <div className="text-xs text-muted-foreground mt-0.5 truncate font-mono">
              {host.identity_file[0]}
            </div>
          )}
          {lastConnected && (
            <div className="text-xs text-muted-foreground mt-0.5">
              Last launched {formatTimeAgo(lastConnected)}
            </div>
          )}
        </div>
        <div className="flex gap-1 opacity-100 transition-opacity shrink-0">
          {isConnectableAlias(host.alias) && (
            <button
              onClick={onConnect}
              className="btn btn-compact"
            >
              Connect
            </button>
          )}
          <button
            disabled={host.read_only}
            onClick={onEdit}
            className="btn btn-compact"
          >
            Edit
          </button>
          <button
            onClick={onDuplicate}
            className="btn btn-compact"
          >
            Duplicate
          </button>
          <button
            disabled={host.read_only}
            onClick={() => setConfirmDelete(true)}
            className="btn btn-danger btn-compact"
          >
            Delete
          </button>
        </div>
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title="Delete Host"
          description={`Delete SSH host '${host.alias}'? This will remove it from ~/.ssh/config.`}
          onConfirm={() => { setConfirmDelete(false); onDelete(); }}
          onCancel={() => setConfirmDelete(false)}
          confirmLabel="Delete"
          destructive
        />
      )}
    </>
  );
}
