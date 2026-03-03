import { useState } from "react";
import type { SshKeyInfo } from "../../lib/tauri";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";

interface KeyCardProps {
  keyInfo: SshKeyInfo;
  agentFingerprints: string[];
  onDelete: () => void;
  onCopyPublicKey: () => void;
  onCopyToServer: () => void;
  onAddToAgent: () => void;
  onRemoveFromAgent: () => void;
}

const ALG_LABELS: Record<string, string> = {
  ed25519: "Ed25519",
  rsa: "RSA",
  ecdsa: "ECDSA",
};

export function KeyCard({ keyInfo, agentFingerprints, onDelete, onCopyPublicKey, onCopyToServer, onAddToAgent, onRemoveFromAgent }: KeyCardProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isInAgent = agentFingerprints.includes(keyInfo.fingerprint);

  return (
    <>
      <div className="flex items-center gap-3 p-3 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors group">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-mono text-sm font-semibold text-foreground">{keyInfo.name}</span>
            <span className="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground font-mono">
              {ALG_LABELS[keyInfo.algorithm] ?? keyInfo.algorithm}
              {keyInfo.bits && ` ${keyInfo.bits}`}
            </span>
            {keyInfo.has_passphrase && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400">
                🔒 passphrase
              </span>
            )}
            {isInAgent && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
                In Agent
              </span>
            )}
          </div>
          <div className="text-xs text-muted-foreground mt-0.5 font-mono truncate">
            {keyInfo.fingerprint}
          </div>
          {keyInfo.comment && (
            <div className="text-xs text-muted-foreground/70 mt-0.5 truncate">{keyInfo.comment}</div>
          )}
          {keyInfo.created_at && (
            <div className="text-xs text-muted-foreground/50 mt-0.5">{keyInfo.created_at}</div>
          )}
        </div>
        <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
          <button
            onClick={onCopyPublicKey}
            className="px-2 py-1 text-xs rounded border border-border hover:bg-accent transition-colors"
          >
            Copy Pub Key
          </button>
          <button
            onClick={onCopyToServer}
            className="px-2 py-1 text-xs rounded border border-border hover:bg-accent transition-colors"
          >
            Copy to Server
          </button>
          {isInAgent ? (
            <button
              onClick={onRemoveFromAgent}
              className="px-2 py-1 text-xs rounded border border-border hover:bg-accent transition-colors"
            >
              Remove from Agent
            </button>
          ) : (
            <button
              onClick={onAddToAgent}
              className="px-2 py-1 text-xs rounded border border-border hover:bg-accent transition-colors"
            >
              Add to Agent
            </button>
          )}
          <button
            onClick={() => setConfirmDelete(true)}
            className="px-2 py-1 text-xs rounded border border-destructive/50 text-destructive hover:bg-destructive/10 transition-colors"
          >
            Delete
          </button>
        </div>
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title="Delete Key"
          description={`Delete key pair '${keyInfo.name}'? Both private and public key files will be removed.`}
          onConfirm={() => { setConfirmDelete(false); onDelete(); }}
          onCancel={() => setConfirmDelete(false)}
          confirmLabel="Delete"
          destructive
        />
      )}
    </>
  );
}
