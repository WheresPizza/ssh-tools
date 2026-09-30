import { useState } from "react";
import { restorePublicKey } from "../../lib/tauri";
import { useStore } from "../../stores";
import type { SshKeyInfo, AgentEnrollment } from "../../lib/tauri";
import { KeyUsageDialog } from "./KeyUsageDialog";

interface KeyCardProps {
  keyInfo: SshKeyInfo;
  tags: string[];
  onDetails: () => void;
  agentFingerprints: string[];
  enrollment?: AgentEnrollment;
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

export function KeyCard({ keyInfo, tags, onDetails, agentFingerprints, enrollment, onDelete, onCopyPublicKey, onCopyToServer, onAddToAgent, onRemoveFromAgent }: KeyCardProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isInAgent = agentFingerprints.includes(keyInfo.fingerprint);

  return (
    <>
      <div className="flex flex-wrap items-center gap-3 p-3 rounded-lg border border-border bg-card hover:bg-accent/40 transition-colors group">
        <div className="flex-1 min-w-[220px]">
          <div className="flex items-center gap-2">
            <span className="font-mono text-sm font-semibold text-foreground">{keyInfo.name}</span>
            <span className="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground font-mono">
              {ALG_LABELS[keyInfo.algorithm] ?? keyInfo.algorithm}
              {keyInfo.bits && ` ${keyInfo.bits}`}
            </span>
            {keyInfo.has_passphrase && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-success/10 text-success">
                passphrase
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
          {enrollment && <p className="text-xs text-muted-foreground mt-1">Last {enrollment.interactive ? "requested in terminal" : "added here"}: {enrollment.lifetime_seconds ? `${Math.ceil(enrollment.lifetime_seconds / 60)} min limit` : "agent default lifetime"}{enrollment.confirm ? "; confirm every use" : ""}. Current restrictions cannot be queried from OpenSSH.</p>}
          {isInAgent && !enrollment && <p className="text-xs text-muted-foreground mt-1">Loaded outside this app session; lifetime and confirmation policy unknown.</p>}
          {keyInfo.error && <p role="status" className="text-xs text-destructive mt-1">{keyInfo.error}</p>}
          {tags.length > 0 && <div className="flex flex-wrap gap-1 mt-2">{tags.map(tag => <span key={tag} className="text-xs rounded border border-border px-1.5 py-0.5 break-all">{tag}</span>)}</div>}
          {keyInfo.comment && (
            <div className="text-xs text-muted-foreground mt-0.5 truncate">{keyInfo.comment}</div>
          )}
          {keyInfo.created_at && (
            <div className="text-xs text-muted-foreground mt-0.5">{keyInfo.created_at}</div>
          )}
        </div>
        <button className="btn btn-compact" onClick={onDetails} aria-label={`Details for ${keyInfo.name}`}>Details</button>
        <fieldset disabled={!!keyInfo.error} className="flex flex-wrap gap-1 opacity-100 transition-opacity">
          {keyInfo.public_key_exists === false && <button className="btn btn-compact" onClick={() => {
            restorePublicKey(keyInfo.private_path, keyInfo.fingerprint).then(() => useStore.getState().showToast("Public companion restored", "success")).catch(e => useStore.getState().showToast(`Cannot restore public key: ${e}`, "error"));
          }}>Restore .pub</button>}
          <button
            onClick={onCopyPublicKey}
            className="btn btn-compact"
          >
            Copy Pub Key
          </button>
          <button
            onClick={onCopyToServer}
            className="btn btn-compact"
          >
            Copy to Server
          </button>
          {isInAgent ? (
            <button
              onClick={onRemoveFromAgent}
              className="btn btn-compact"
            >
              Remove from Agent
            </button>
          ) : (
            <button
              onClick={onAddToAgent}
              className="btn btn-compact"
            >
              Add to Agent
            </button>
          )}
          <button
            onClick={() => setConfirmDelete(true)}
            className="btn btn-danger btn-compact"
          >
            Delete
          </button>
        </fieldset>
      </div>

      {confirmDelete && (
        <KeyUsageDialog
          keyInfo={keyInfo}
          onDelete={() => { setConfirmDelete(false); onDelete(); }}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </>
  );
}
