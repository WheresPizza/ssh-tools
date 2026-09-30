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
  listKeyMetadata: vi.fn(), saveKeyMetadata: vi.fn(), auditSshKeys: vi.fn(),
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
  vi.mocked(api.listKeyMetadata).mockResolvedValue({ entries: {}, revision: "initial" });
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

describe("key details, annotations and audit", () => {
  it("edits shared annotations and searches by tag after returning to the library", async () => {
    vi.mocked(api.saveKeyMetadata).mockResolvedValue({ entries: { [key.fingerprint]: { tags: ["client"], purpose: "Deploy", note: "", replace_on: null } }, revision: "saved" });
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByRole("button", { name: `Details for ${key.name}` }));
    expect(await screen.findByText(/No project folders selected/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText(/Tags/), { target: { value: "client" } });
    fireEvent.change(screen.getByLabelText("Purpose"), { target: { value: "Deploy" } });
    fireEvent.click(screen.getByRole("button", { name: "Save annotations" }));
    await waitFor(() => expect(api.saveKeyMetadata).toHaveBeenCalledWith(key.private_path, key.fingerprint, { tags: ["client"], purpose: "Deploy", note: "", replace_on: null }, "initial"));
    await waitFor(() => expect((screen.getByRole("button", { name: "Save annotations" }) as HTMLButtonElement).disabled).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "Back to keys" }));
    fireEvent.change(screen.getByLabelText("Search keys"), { target: { value: "client" } });
    expect(screen.getByRole("button", { name: `Details for ${key.name}` })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Search keys"), { target: { value: "unknown-tag" } });
    expect(screen.getByText("No matching keys")).toBeTruthy();
  });
  it("preserves a dirty draft across refresh and stale-save rejection, then confirms discard", async () => {
    vi.mocked(api.saveKeyMetadata).mockRejectedValue(new Error("Key metadata changed. Reload before saving your edits."));
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByRole("button", { name: `Details for ${key.name}` }));
    fireEvent.change(screen.getByLabelText("Notes"), { target: { value: "Do not lose this draft" } });
    vi.mocked(api.listKeyMetadata).mockResolvedValue({ entries: {}, revision: "external-edit" });
    fireEvent.focus(window);
    await waitFor(() => expect(api.listKeyMetadata).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole("button", { name: "Save annotations" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect((screen.getByLabelText("Notes") as HTMLTextAreaElement).value).toBe("Do not lose this draft");
    expect(vi.mocked(api.saveKeyMetadata).mock.calls[0][3]).toBe("initial");
    fireEvent.click(screen.getByRole("button", { name: "Back to keys" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    expect((screen.getByLabelText("Notes") as HTMLTextAreaElement).value).toBe("Do not lose this draft");
  });
  it("can inspect malformed keys while annotation writes remain disabled", async () => {
    vi.mocked(api.listSshKeys).mockResolvedValue([{ ...key, error: "Broken key", fingerprint: "" }]);
    render(<SshKeysPage />);
    fireEvent.click(await screen.findByRole("button", { name: `Details for ${key.name}` }));
    expect(screen.getByText(/Annotations are unavailable until/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Save annotations" }) as HTMLButtonElement).disabled).toBe(true);
    expect(api.saveKeyMetadata).not.toHaveBeenCalled();
  });
  it("runs audit explicitly, shows partial coverage, filters findings and opens details", async () => {
    vi.mocked(api.auditSshKeys).mockResolvedValue({ checked_at: 1790000000, warnings: ["Permission checks incomplete"], entries: [{ key_path: key.private_path, fingerprint: key.fingerprint, copies: [key.private_path], usage: null, findings: [{ code: "no-passphrase", severity: "warning", title: "No passphrase", detail: "Private file is not encrypted" }] }] });
    render(<SshKeysPage />);
    await screen.findByRole("button", { name: `Details for ${key.name}` });
    expect(api.auditSshKeys).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Audit keys" }));
    expect(await screen.findByText("warning · No passphrase")).toBeTruthy();
    expect(screen.getByText("Permission checks incomplete")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Findings"), { target: { value: "error" } });
    expect(screen.getByText("No findings match this filter.")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Findings"), { target: { value: "all" } });
    fireEvent.click(screen.getByRole("button", { name: "Key details" }));
    expect(await screen.findByText("References to this file")).toBeTruthy();
    expect(api.fixPermissions).not.toHaveBeenCalled();
  });
  it("keeps the key library usable when metadata cannot be read", async () => {
    vi.mocked(api.listKeyMetadata).mockRejectedValue(new Error("Malformed metadata"));
    render(<SshKeysPage />);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(await screen.findByRole("button", { name: `Details for ${key.name}` })).toBeTruthy();
  });
});

it("adds a due-date reminder to audit without claiming that the key expired", async () => {
  vi.mocked(api.listKeyMetadata).mockResolvedValue({ entries: { [key.fingerprint]: { tags: [], purpose: "", note: "", replace_on: "2000-01-01" } }, revision: "due" });
  vi.mocked(api.auditSshKeys).mockResolvedValue({ checked_at: 1790000000, warnings: [], entries: [{ key_path: key.private_path, fingerprint: key.fingerprint, copies: [key.private_path], usage: null, findings: [] }] });
  render(<SshKeysPage />);
  fireEvent.click(await screen.findByRole("button", { name: "Audit keys" }));
  expect(await screen.findByText("warning · Planned replacement is due")).toBeTruthy();
  expect(screen.getByText(/This does not expire or revoke the key/)).toBeTruthy();
  expect(api.deleteSshKey).not.toHaveBeenCalled();
});
