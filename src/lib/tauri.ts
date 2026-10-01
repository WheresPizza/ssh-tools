import { invoke } from "@tauri-apps/api/core";

export interface SshHost {
  revision?: string;
  source_path?: string;
  read_only?: boolean;
  can_reorder?: boolean;
  alias: string;
  hostname: string | null;
  user: string | null;
  port: number | null;
  identity_file: string[];
  proxy_jump: string | null;
  forward_agent: boolean | null;
  server_alive_interval: number | null;
  extra_fields: [string, string][];
  line_start: number;
  line_end: number;
}

export interface SshKeyInfo {
  name: string;
  private_path: string;
  public_path: string;
  public_key_exists?: boolean;
  algorithm: string;
  bits: number | null;
  fingerprint: string;
  comment: string | null;
  has_passphrase: boolean | null;
  error?: string | null;
  created_at: string | null;
}

export type KeyAlgorithm = "Ed25519" | "Rsa2048" | "Rsa4096" | "EcdsaP256" | "EcdsaP384";

export interface KeyGenParams {
  algorithm: KeyAlgorithm;
  comment: string;
  filename: string;
  passphrase: string | null;
}

export interface KnownHostEntry {
  line_number: number;
  hostname: string;
  key_type: string;
  key_data: string;
  marker?: string | null;
}

export interface TerminalInfo {
  name: string;
  path: string;
  is_preferred: boolean;
}

export interface AppConfig {
  preferred_terminal: string | null;
  theme: string;
}

// SSH Config
export const getSshConfig = () => invoke<SshHost[]>("get_ssh_config");
export const addHost = (host: SshHost) => invoke<void>("add_host", { host });
export const updateHost = (originalAlias: string, host: SshHost) =>
  invoke<void>("update_host", { original_alias: originalAlias, host });
export const deleteHost = (alias: string, revision: string, sourcePath = "", lineStart: number | null = null) => invoke<void>("delete_host", { alias, revision, source_path: sourcePath, line_start: lineStart });
export const reorderHosts = (aliases: string[], revision: string, sourcePath = "") => invoke<void>("reorder_hosts", { aliases, revision, source_path: sourcePath });

// SSH Keys
export const listSshKeys = () => invoke<SshKeyInfo[]>("list_ssh_keys");
export const generateSshKey = (params: KeyGenParams) => invoke<SshKeyInfo>("generate_ssh_key", { params });
export const getPublicKey = (keyPath: string) => invoke<string>("get_public_key", { key_path: keyPath });
export const deleteSshKey = (keyPath: string, expectedFingerprint: string) => invoke<void>("delete_ssh_key", { key_path: keyPath, expected_fingerprint: expectedFingerprint });
export const getKeyFingerprint = (keyPath: string) => invoke<string>("get_key_fingerprint", { key_path: keyPath });

export const listAgentKeys = () => invoke<string[]>("list_agent_keys");
export const addKeyToAgent = (keyPath: string, options: AgentOptions | null = null) => invoke<boolean>("add_key_to_agent", { key_path: keyPath, options });
export const removeKeyFromAgent = (keyPath: string) => invoke<void>("remove_key_from_agent", { key_path: keyPath });

// Known Hosts
export const listKnownHosts = () => invoke<KnownHostEntry[]>("list_known_hosts");
export const deleteKnownHosts = (entries: KnownHostEntry[]) => invoke<void>("delete_known_hosts", { entries });
export const verifyKnownHost = (hostname: string, keyType: string, storedKeyData: string) =>
  invoke<boolean>("verify_known_host", { hostname, key_type: keyType, stored_key_data: storedKeyData });

// Launcher
export const launchSshConnection = (hostAlias: string) =>
  invoke<void>("launch_ssh_connection", { host_alias: hostAlias });
export const getDetectedTerminal = () => invoke<TerminalInfo[]>("get_detected_terminal");
export const setPreferredTerminal = (terminal: string) =>
  invoke<void>("set_preferred_terminal", { terminal });
export const copyKeyToServer = (keyPath: string, hostAlias: string) =>
  invoke<void>("copy_key_to_server", { key_path: keyPath, host_alias: hostAlias });

// App
export const getAppConfig = () => invoke<AppConfig>("get_app_config");
export const saveAppConfig = (config: AppConfig) => invoke<void>("save_app_config", { config });
export const getSshDirPath = () => invoke<string>("get_ssh_dir_path");

export interface PermissionIssue { path: string; current: string; expected: string }
export const auditPermissions = () => invoke<PermissionIssue[]>("audit_permissions");
export const fixPermissions = () => invoke<PermissionIssue[]>("fix_permissions");

export interface BackupInfo { path: string; current_revision: string; backup_revision: string; preview: string }
export const listBackups = () => invoke<BackupInfo[]>("list_backups");
export const restoreBackup = (backup: BackupInfo) => invoke<void>("restore_backup", { path: backup.path, current_revision: backup.current_revision, backup_revision: backup.backup_revision });
export const getWorkspace = () => invoke<{ ssh_dir: string; isolated: boolean }>("get_workspace");

export interface GitProfile { name: string; account: string; alias: string; hostname: string; user: string; port: number; key_path: string }
export interface ProfileSnapshot { profiles: GitProfile[]; revision: string }
export const listGitProfiles = () => invoke<ProfileSnapshot>("list_git_profiles");
export const saveGitProfile = (profile: GitProfile, originalAlias: string | null, revision: string) => invoke<ProfileSnapshot>("save_git_profile", { profile, original_alias: originalAlias, revision });
export const deleteGitProfile = (alias: string, revision: string) => invoke<ProfileSnapshot>("delete_git_profile", { alias, revision });

export interface RepositoryRemote { repository: string; name: string; direction: string; url: string; ssh_host: string | null; ssh_user: string | null; ssh_port: number | null; profiles: string[]; key_paths: string[]; warnings: string[] }
export interface RepositoryScan { roots: string[]; remotes: RepositoryRemote[]; warnings: string[]; repositories: number }
export interface KeyUsage { hosts: string[]; profiles: string[]; repositories: string[]; roots: string[]; warnings: string[] }
export const setRepositoryRoots = (roots: string[]) => invoke<void>("set_repository_roots", { roots });
export const scanRepositories = () => invoke<RepositoryScan>("scan_repositories");
export const getKeyUsage = (keyPath: string) => invoke<KeyUsage>("get_key_usage", { key_path: keyPath });

export interface DiagnosticStep { status: "ok" | "warning" | "error"; title: string; detail: string; suggestion: string | null }
export interface DiagnosticReport { alias: string; config_path: string; hostname: string | null; user: string | null; port: string | null; identities_only: boolean; identity_files: string[]; agent_fingerprints: string[]; authenticated: boolean | null; steps: DiagnosticStep[] }
export const diagnoseSsh = (alias: string, network: boolean, port: number | null = null) => invoke<DiagnosticReport>("diagnose_ssh", { alias, network, port });

export interface ImportInspection { key: SshKeyInfo; duplicates: string[]; public_recovered: boolean; revision: string; warning: string | null }
export const inspectKeyImport = (sourcePath: string) => invoke<ImportInspection>("inspect_key_import", { source_path: sourcePath });
export const importSshKey = (sourcePath: string, filename: string, revision: string) => invoke<SshKeyInfo>("import_ssh_key", { source_path: sourcePath, filename, revision });
export const restorePublicKey = (keyPath: string, expectedFingerprint: string) => invoke<void>("restore_public_key", { key_path: keyPath, expected_fingerprint: expectedFingerprint });

export interface AgentOptions { lifetime_seconds: number | null; confirm: boolean }
export interface AgentEnrollment extends AgentOptions { fingerprint: string; requested_at: number; interactive: boolean }
export const listAgentEnrollments = () => invoke<AgentEnrollment[]>("list_agent_enrollments");

// Local annotations are shared by fingerprint, never written into key files.
export interface KeyMetadata { tags: string[]; purpose: string; note: string; replace_on: string | null }
export interface MetadataSnapshot { entries: Record<string, KeyMetadata>; revision: string }
export const listKeyMetadata = () => invoke<MetadataSnapshot>("list_key_metadata");
export const saveKeyMetadata = (keyPath: string, expectedFingerprint: string, metadata: KeyMetadata, revision: string) =>
  invoke<MetadataSnapshot>("save_key_metadata", { key_path: keyPath, expected_fingerprint: expectedFingerprint, metadata, revision });
export interface KeyFinding { code: string; severity: "error" | "warning" | "info"; title: string; detail: string }
export interface KeyAuditEntry { key_path: string; fingerprint: string; findings: KeyFinding[]; usage: KeyUsage | null; copies: string[] }
export interface KeyAudit { entries: KeyAuditEntry[]; warnings: string[]; checked_at: number }
export const auditSshKeys = () => invoke<KeyAudit>("audit_ssh_keys");

export interface RepositoryAccessPlan {
  repository: string; remote: string; before: string; after: string; key_path: string;
  fingerprint: string; alias: string; revision: string; ssh_revision: string; warnings: string[];
}
export const inspectRepository = (repository: string) => invoke<RepositoryScan>("inspect_repository", { repository });
export const previewRepositoryAccess = (repository: string, remote: string, key: SshKeyInfo, alias: string) => invoke<RepositoryAccessPlan>("preview_repository_access", { repository, remote, key_path: key.private_path, fingerprint: key.fingerprint, alias });
export const applyRepositoryAccess = (plan: RepositoryAccessPlan) => invoke<void>("apply_repository_access", { plan });

export interface KeyRotationTarget { id: string; kind: "host" | "profile"; label: string; source: string; line: number; before: string; after: string; blocked: string | null }
export interface KeyRotationPlan { old_path: string; old_fingerprint: string; new_path: string; new_fingerprint: string; revisions: Record<string, string>; targets: KeyRotationTarget[]; warnings: string[] }
export const previewKeyRotation = (oldKey: SshKeyInfo, newKey: SshKeyInfo) => invoke<KeyRotationPlan>("preview_key_rotation", { old_path: oldKey.private_path, old_fingerprint: oldKey.fingerprint, new_path: newKey.private_path, new_fingerprint: newKey.fingerprint });
export const applyKeyRotation = (plan: KeyRotationPlan, selected: string[]) => invoke<void>("apply_key_rotation", { plan, selected });
