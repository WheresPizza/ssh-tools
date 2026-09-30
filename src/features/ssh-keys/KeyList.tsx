import type { SshKeyInfo, AgentEnrollment } from "../../lib/tauri";
import { KeyCard } from "./KeyCard";

interface KeyListProps {
  keys: SshKeyInfo[];
  agentFingerprints: string[];
  enrollments: AgentEnrollment[];
  onDelete: (keyPath: string, name: string, fingerprint: string) => void;
  onCopyPublicKey: (keyPath: string) => void;
  onCopyToServer: (key: SshKeyInfo) => void;
  onAddToAgent: (key: SshKeyInfo) => void;
  onRemoveFromAgent: (key: SshKeyInfo) => void;
}

export function KeyList({ keys, agentFingerprints, enrollments, onDelete, onCopyPublicKey, onCopyToServer, onAddToAgent, onRemoveFromAgent }: KeyListProps) {
  return (
    <div className="flex flex-col gap-2">
      {keys.map((key) => (
        <KeyCard
          key={key.private_path}
          keyInfo={key}
          agentFingerprints={agentFingerprints}
          enrollment={enrollments.find(e => e.fingerprint === key.fingerprint)}
          onDelete={() => onDelete(key.private_path, key.name, key.fingerprint)}
          onCopyPublicKey={() => onCopyPublicKey(key.private_path)}
          onCopyToServer={() => onCopyToServer(key)}
          onAddToAgent={() => onAddToAgent(key)}
          onRemoveFromAgent={() => onRemoveFromAgent(key)}
        />
      ))}
    </div>
  );
}
