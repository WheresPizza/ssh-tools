import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import {
  getSshConfig, addHost, updateHost, deleteHost, reorderHosts,
  listSshKeys, generateSshKey, getPublicKey, deleteSshKey, getKeyFingerprint,
  listKnownHosts, deleteKnownHost, deleteKnownHostByHostname,
  launchSshConnection, getDetectedTerminal, setPreferredTerminal,
  getAppConfig, saveAppConfig, getSshDirPath,
} from "@/lib/tauri";
import type { SshHost, KeyGenParams, AppConfig } from "@/lib/tauri";

function makeHost(o: Partial<SshHost> = {}): SshHost {
  return {
    alias: "myserver", hostname: "192.168.1.1", user: "alice", port: 22,
    identity_file: [], proxy_jump: null, forward_agent: null,
    server_alive_interval: null, extra_fields: [], line_start: 0, line_end: 2,
    ...o,
  };
}
function makeParams(o: Partial<KeyGenParams> = {}): KeyGenParams {
  return { algorithm: "Ed25519", comment: "alice@laptop", filename: "id_ed25519_test", passphrase: null, ...o };
}
function makeConfig(o: Partial<AppConfig> = {}): AppConfig {
  return { preferred_terminal: null, theme: "system", ...o };
}

describe("IPC contract: tauri.ts", () => {
  beforeEach(() => { mockInvoke.mockReset(); mockInvoke.mockResolvedValue(undefined); });

  // ── SSH Config ──────────────────────────────────────────────────────────────
  it("getSshConfig — no args", async () => {
    mockInvoke.mockResolvedValue([]);
    await getSshConfig();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_ssh_config");
  });
  it("addHost — passes host", async () => {
    const host = makeHost();
    await addHost(host);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("add_host", { host });
  });
  it("updateHost — sends original_alias (snake_case)", async () => {
    const host = makeHost();
    await updateHost("myserver", host);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("update_host", { original_alias: "myserver", host });
  });
  it("updateHost — does NOT send originalAlias (camelCase)", async () => {
    await updateHost("myserver", makeHost());
    const [, args] = mockInvoke.mock.calls[0];
    expect(args).not.toHaveProperty("originalAlias");
  });
  it("deleteHost — sends alias", async () => {
    await deleteHost("myserver");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("delete_host", { alias: "myserver" });
  });
  it("reorderHosts — sends aliases array", async () => {
    const aliases = ["b", "a"];
    await reorderHosts(aliases);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("reorder_hosts", { aliases });
  });

  // ── SSH Keys ────────────────────────────────────────────────────────────────
  it("listSshKeys — no args", async () => {
    mockInvoke.mockResolvedValue([]);
    await listSshKeys();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("list_ssh_keys");
  });
  it("generateSshKey — wraps params", async () => {
    const params = makeParams();
    await generateSshKey(params);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("generate_ssh_key", { params });
  });
  it("getPublicKey — sends key_path (snake_case)", async () => {
    mockInvoke.mockResolvedValue("ssh-ed25519 AAAA");
    await getPublicKey("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_public_key", { key_path: "/home/alice/.ssh/id_ed25519" });
  });
  it("getPublicKey — does NOT send keyPath (camelCase)", async () => {
    await getPublicKey("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke.mock.calls[0][1]).not.toHaveProperty("keyPath");
  });
  it("deleteSshKey — sends key_path (snake_case)", async () => {
    await deleteSshKey("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("delete_ssh_key", { key_path: "/home/alice/.ssh/id_ed25519" });
  });
  it("deleteSshKey — does NOT send keyPath", async () => {
    await deleteSshKey("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke.mock.calls[0][1]).not.toHaveProperty("keyPath");
  });
  it("getKeyFingerprint — sends key_path (snake_case)", async () => {
    mockInvoke.mockResolvedValue("SHA256:abc");
    await getKeyFingerprint("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_key_fingerprint", { key_path: "/home/alice/.ssh/id_ed25519" });
  });
  it("getKeyFingerprint — does NOT send keyPath", async () => {
    await getKeyFingerprint("/home/alice/.ssh/id_ed25519");
    expect(mockInvoke.mock.calls[0][1]).not.toHaveProperty("keyPath");
  });

  // ── Known Hosts ─────────────────────────────────────────────────────────────
  it("listKnownHosts — no args", async () => {
    mockInvoke.mockResolvedValue([]);
    await listKnownHosts();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("list_known_hosts");
  });
  it("deleteKnownHost — sends line_number (snake_case)", async () => {
    await deleteKnownHost(42);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("delete_known_host", { line_number: 42 });
  });
  it("deleteKnownHost — does NOT send lineNumber (camelCase)", async () => {
    await deleteKnownHost(42);
    expect(mockInvoke.mock.calls[0][1]).not.toHaveProperty("lineNumber");
  });
  it("deleteKnownHostByHostname — sends hostname", async () => {
    await deleteKnownHostByHostname("github.com");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("delete_known_host_by_hostname", { hostname: "github.com" });
  });

  // ── Launcher ────────────────────────────────────────────────────────────────
  it("launchSshConnection — sends host_alias (snake_case) — the original bug", async () => {
    await launchSshConnection("myserver");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("launch_ssh_connection", { host_alias: "myserver" });
  });
  it("launchSshConnection — does NOT send hostAlias (camelCase)", async () => {
    await launchSshConnection("myserver");
    expect(mockInvoke.mock.calls[0][1]).not.toHaveProperty("hostAlias");
  });
  it("getDetectedTerminal — no args", async () => {
    mockInvoke.mockResolvedValue([]);
    await getDetectedTerminal();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_detected_terminal");
  });
  it("setPreferredTerminal — sends terminal", async () => {
    await setPreferredTerminal("iTerm2");
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("set_preferred_terminal", { terminal: "iTerm2" });
  });

  // ── App ─────────────────────────────────────────────────────────────────────
  it("getAppConfig — no args", async () => {
    mockInvoke.mockResolvedValue(makeConfig());
    await getAppConfig();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_app_config");
  });
  it("saveAppConfig — passes config", async () => {
    const config = makeConfig({ theme: "dark" });
    await saveAppConfig(config);
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("save_app_config", { config });
  });
  it("getSshDirPath — no args", async () => {
    mockInvoke.mockResolvedValue("/home/alice/.ssh");
    await getSshDirPath();
    expect(mockInvoke).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith("get_ssh_dir_path");
  });
});
