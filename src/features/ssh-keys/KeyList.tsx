import type { SshKeyInfo } from "../../lib/tauri";
import { KeyCard } from "./KeyCard";

interface KeyListProps {
  keys: SshKeyInfo[];
  agentFingerprints: string[];
  onDelete: (keyPath: string, name: string) => void;
  onCopyPublicKey: (keyPath: string) => void;
  onCopyToServer: (key: SshKeyInfo) => void;
  onAddToAgent: (key: SshKeyInfo) => void;
  onRemoveFromAgent: (key: SshKeyInfo) => void;
}

export function KeyList({ keys, agentFingerprints, onDelete, onCopyPublicKey, onCopyToServer, onAddToAgent, onRemoveFromAgent }: KeyListProps) {
  return (
    <div className="flex flex-col gap-2">
      {keys.map((key) => (
        <KeyCard
          key={key.private_path}
          keyInfo={key}
          agentFingerprints={agentFingerprints}
          onDelete={() => onDelete(key.private_path, key.name)}
          onCopyPublicKey={() => onCopyPublicKey(key.private_path)}
          onCopyToServer={() => onCopyToServer(key)}
          onAddToAgent={() => onAddToAgent(key)}
          onRemoveFromAgent={() => onRemoveFromAgent(key)}
        />
      ))}
    </div>
  );
}
