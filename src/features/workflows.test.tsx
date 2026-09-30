// @vitest-environment jsdom
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SshKeysPage } from "./ssh-keys/SshKeysPage";
import { KnownHostsPage } from "./known-hosts/KnownHostsPage";
import { BackupPanel } from "../components/common/BackupPanel";
import { KeyGeneratorDialog } from "./ssh-keys/KeyGeneratorDialog";
import { useStore } from "../stores";
import * as api from "../lib/tauri";

vi.mock("../lib/tauri", () => ({
  listAgentEnrollments: vi.fn(), getKeyUsage: vi.fn(), listSshKeys: vi.fn(), getSshConfig: vi.fn(), listAgentKeys: vi.fn(),
  addKeyToAgent: vi.fn(), removeKeyFromAgent: vi.fn(), deleteSshKey: vi.fn(),
  getPublicKey: vi.fn(), copyKeyToServer: vi.fn(), generateSshKey: vi.fn(),
  auditPermissions: vi.fn(), fixPermissions: vi.fn(), listKnownHosts: vi.fn(),
  deleteKnownHosts: vi.fn(), verifyKnownHost: vi.fn(), listBackups: vi.fn(), restoreBackup: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));

const key = { name: "github-work", private_path: "/fixture/.ssh/github-work", public_path: "/fixture/.ssh/github-work.pub", algorithm: "ed25519", bits: 256, fingerprint: "SHA256:fixture", comment: "work", has_passphrase: true, created_at: null };
const host = { alias: "github-work", hostname: "github.com", user: "git", port: null, identity_file: [], proxy_jump: null, forward_agent: null, server_alive_interval: null, extra_fields: [], line_start: 0, line_end: 2 };
const entry = { line_number: 1, hostname: "github.com", key_type: "ssh-ed25519", key_data: "AAAA", marker: null };

beforeEach(() => {
  vi.clearAllMocks();
  useStore.persist.setOptions({ storage: { getItem: () => null, setItem: () => {}, removeItem: () => {} } });
  useStore.setState({ hosts: [], keys: [], knownHosts: [], toast: null });
  vi.mocked(api.getKeyUsage).mockResolvedValue({ hosts: [], profiles: [], repositories: [], roots: [], warnings: [] });
  vi.mocked(api.listSshKeys).mockResolvedValue([key]);
  vi.mocked(api.getSshConfig).mockResolvedValue([host]);
  vi.mocked(api.listAgentKeys).mockResolvedValue([]);
  vi.mocked(api.listAgentEnrollments).mockResolvedValue([]);
  vi.mocked(api.auditPermissions).mockResolvedValue([]);
  vi.mocked(api.listKnownHosts).mockResolvedValue([entry]);
  vi.mocked(api.listBackups).mockResolvedValue([]);
  vi.mocked(api.deleteKnownHosts).mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("SSH management workflows", () => {
  it("loads destinations when opening Keys directly and only opens transfer after confirmation", async () => {
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByText("Copy to Server"));
    expect(await screen.findByRole("option", { name: "github-work (github.com)" })).toBeTruthy();
    expect(api.copyKeyToServer).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Copy Key" }));
    await waitFor(() => expect(api.copyKeyToServer).toHaveBeenCalledWith(key.private_path, host.alias));
  });
  it("reports passphrase entry rather than falsely claiming an encrypted key was added", async () => {
    vi.mocked(api.addKeyToAgent).mockResolvedValue(false);
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByText("Add to Agent"));
    fireEvent.click(screen.getByRole("button", { name: "Add with these settings" }));
    await waitFor(() => expect(useStore.getState().toast?.message).toContain("Enter the key passphrase"));
    const before = vi.mocked(api.listAgentKeys).mock.calls.length;
    fireEvent.focus(window);
    await waitFor(() => expect(vi.mocked(api.listAgentKeys).mock.calls.length).toBeGreaterThan(before));
  });
  it("requires confirmation and passes the expected known-host record to deletion", async () => {
    render(<KnownHostsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(api.deleteKnownHosts).not.toHaveBeenCalled();
    expect(screen.getByText("Remove trusted host entries")).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(api.deleteKnownHosts).toHaveBeenCalledWith([entry]));
  });
});


describe("recovery and key safety", () => {
  it("confirms key deletion against the displayed fingerprint", async () => {
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    expect(api.deleteSshKey).not.toHaveBeenCalled();
    fireEvent.click(await within(screen.getByRole("dialog")).findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(api.deleteSshKey).toHaveBeenCalledWith(key.private_path, key.fingerprint));
  });
  it("shows agent unavailability instead of treating it as an empty connected agent", async () => {
    vi.mocked(api.listAgentKeys).mockRejectedValue(new Error("SSH Agent is not available"));
    render(<SshKeysPage />);
    expect(await screen.findByText(/SSH Agent is not available/)).toBeTruthy();
  });
  it("requires review and confirmation before restoring the selected backup", async () => {
    const backup = { path: "/fixture/ssh/config", current_revision: "current", backup_revision: "backup", preview: "Host previous" };
    vi.mocked(api.listBackups).mockResolvedValue([backup]);
    const restored = vi.fn();
    render(<BackupPanel onRestored={restored} />);
    fireEvent.click(await screen.findByText(backup.path));
    expect(screen.getByText("Host previous")).toBeTruthy();
    fireEvent.click(screen.getByText("Restore this version"));
    expect(api.restoreBackup).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(api.restoreBackup).toHaveBeenCalledWith(backup));
    await waitFor(() => expect(restored).toHaveBeenCalledOnce());
  });
  it("keeps the generation form open after failure and rejects mismatched passphrases", async () => {
    const closed = vi.fn();
    render(<KeyGeneratorDialog onClose={closed} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Passphrase (optional)"), { target: { value: "fixture" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    expect(api.generateSshKey).not.toHaveBeenCalled();
    expect(useStore.getState().toast?.message).toBe("Passphrases do not match");
    fireEvent.change(screen.getByLabelText("Confirm Passphrase"), { target: { value: "fixture" } });
    vi.mocked(api.generateSshKey).mockRejectedValue(new Error("File already exists"));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() => expect(useStore.getState().toast?.message).toContain("File already exists"));
    expect(closed).not.toHaveBeenCalled();
  });
});
