import { invoke } from "@tauri-apps/api/core";

export interface SshHost {
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
  algorithm: string;
  bits: number | null;
  fingerprint: string;
  comment: string | null;
  has_passphrase: boolean;
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
export const deleteHost = (alias: string) => invoke<void>("delete_host", { alias });
export const reorderHosts = (aliases: string[]) => invoke<void>("reorder_hosts", { aliases });

// SSH Keys
export const listSshKeys = () => invoke<SshKeyInfo[]>("list_ssh_keys");
export const generateSshKey = (params: KeyGenParams) => invoke<SshKeyInfo>("generate_ssh_key", { params });
export const getPublicKey = (keyPath: string) => invoke<string>("get_public_key", { key_path: keyPath });
export const deleteSshKey = (keyPath: string) => invoke<void>("delete_ssh_key", { key_path: keyPath });
export const getKeyFingerprint = (keyPath: string) => invoke<string>("get_key_fingerprint", { key_path: keyPath });

export const listAgentKeys = () => invoke<string[]>("list_agent_keys");
export const addKeyToAgent = (keyPath: string) => invoke<void>("add_key_to_agent", { key_path: keyPath });
export const removeKeyFromAgent = (keyPath: string) => invoke<void>("remove_key_from_agent", { key_path: keyPath });

// Known Hosts
export const listKnownHosts = () => invoke<KnownHostEntry[]>("list_known_hosts");
export const deleteKnownHost = (lineNumber: number) => invoke<void>("delete_known_host", { line_number: lineNumber });
export const deleteKnownHostByHostname = (hostname: string) =>
  invoke<void>("delete_known_host_by_hostname", { hostname });
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
