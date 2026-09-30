import { useCallback, useEffect, useState } from "react";
import { getKeyUsage, type KeyUsage, type SshKeyInfo } from "../../lib/tauri";
import { ConfirmDialog } from "../../components/common/ConfirmDialog";

export function KeyUsageDialog({ keyInfo, onDelete, onCancel }: { keyInfo: SshKeyInfo; onDelete: () => void; onCancel: () => void }) {
  const [usage, setUsage] = useState<KeyUsage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setUsage(await getKeyUsage(keyInfo.private_path)); setError(null); }
    catch (e) { setError(String(e)); }
  }, [keyInfo.private_path]);
  useEffect(() => { void load(); }, [load]);
  if (!usage) return <ConfirmDialog title="Check key usage" description={error ? `Cannot check dependencies: ${error}` : "Checking SSH hosts, Git profiles and selected project folders…"} confirmLabel={error ? "Retry check" : "Checking…"} confirmDisabled={!error} onConfirm={load} onCancel={onCancel} />;
  const references = [...usage.profiles.map(name => `Profile: ${name}`), ...usage.hosts.map(name => `Host: ${name}`), ...usage.repositories.map(name => `Repository: ${name}`)];
  const description = [
    `Delete '${keyInfo.name}'? Both private and public key files will be removed.`,
    references.length ? `These configured references may stop working:\n${references.join("\n")}` : "No explicit references found in the checked configuration and folders.",
    usage.roots.length ? `Checked project folders: ${usage.roots.join(", ")}` : "No project folders selected; repository usage has not been checked.",
    ...usage.warnings,
    "Unscanned repositories, dynamic SSH rules and keys installed on servers are outside this check. Deletion does not revoke remote access.",
  ].join("\n\n");
  return <ConfirmDialog title="Delete Key" description={description} confirmLabel="Delete" destructive onConfirm={onDelete} onCancel={onCancel} />;
}
