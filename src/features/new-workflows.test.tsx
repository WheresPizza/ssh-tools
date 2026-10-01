// @vitest-environment jsdom
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup, within } from "@testing-library/react";
import { GitProfilesPage } from "./git-profiles/GitProfilesPage";
import { KeyRepositories } from "./ssh-keys/KeyRepositories";
import { DiagnosticsPage } from "./diagnostics/DiagnosticsPage";
import { KeyImportDialog } from "./ssh-keys/KeyImportDialog";
import { AgentOptionsDialog } from "./ssh-keys/AgentOptionsDialog";
import { KeyUsageDialog } from "./ssh-keys/KeyUsageDialog";
import { useStore } from "../stores";
import * as api from "../lib/tauri";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
vi.mock("../lib/tauri", () => ({ listGitProfiles: vi.fn(), saveGitProfile: vi.fn(), deleteGitProfile: vi.fn(), listSshKeys: vi.fn(), inspectRepository: vi.fn(), previewRepositoryAccess: vi.fn(), applyRepositoryAccess: vi.fn(), scanRepositories: vi.fn(), setRepositoryRoots: vi.fn(), getSshConfig: vi.fn(), diagnoseSsh: vi.fn(), inspectKeyImport: vi.fn(), importSshKey: vi.fn(), getKeyUsage: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));
const key: api.SshKeyInfo = { name: "work", private_path: "/fixture/ssh/work", public_path: "/fixture/ssh/work.pub", algorithm: "ed25519", bits: 256, fingerprint: "SHA256:fixture", comment: "Work", has_passphrase: true, error: null, created_at: null };
const profile: api.GitProfile = { name: "Work", account: "alice", alias: "git-work", hostname: "github.com", user: "git", port: 22, key_path: key.private_path };
const snapshot = { profiles: [profile], revision: "loaded-revision" };
const report: api.DiagnosticReport = { alias: profile.alias, config_path: "/fixture/ssh/config", hostname: profile.hostname, user: "git", port: "22", identities_only: true, identity_files: [key.private_path], agent_fingerprints: [], authenticated: null, steps: [] };
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listen).mockResolvedValue(() => {});
  useStore.persist.setOptions({ storage: { getItem: () => null, setItem: () => {}, removeItem: () => {} } });
  useStore.setState({ toast: null, diagnosticAlias: profile.alias, diagnosticPort: null });
  vi.mocked(api.listGitProfiles).mockResolvedValue(snapshot);
  vi.mocked(api.listSshKeys).mockResolvedValue([key]);
  vi.mocked(api.getSshConfig).mockResolvedValue([]);
  vi.mocked(api.scanRepositories).mockResolvedValue({ roots: [], remotes: [], warnings: [], repositories: 0 });
  vi.mocked(api.diagnoseSsh).mockResolvedValue(report);
});
afterEach(cleanup);
describe("Git profiles and repository safety", () => {
  it("copies an account-specific clone URL and saves edits against the loaded revision", async () => {
    vi.mocked(api.saveGitProfile).mockResolvedValue(snapshot);
    render(<GitProfilesPage />);
    await screen.findByText("Work");
    fireEvent.change(screen.getByLabelText("Repository path"), { target: { value: "org/project" } });
    fireEvent.click(screen.getByRole("button", { name: "Copy clone URL" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("git@git-work:org/project.git"));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Profile name"), { target: { value: "Client" } });
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    await waitFor(() => expect(api.saveGitProfile).toHaveBeenCalledWith({ ...profile, name: "Client" }, "git-work", "loaded-revision"));
  });
  it("keeps a stale profile edit open and reports its rejection", async () => {
    vi.mocked(api.saveGitProfile).mockRejectedValue(new Error("SSH config changed"));
    render(<GitProfilesPage />); fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    expect(await screen.findByRole("alert")).toBeTruthy(); expect(screen.getByLabelText("SSH key")).toBeTruthy();
  });
  it("adds only the explicitly selected project folder", async () => {
    vi.mocked(open).mockResolvedValue(["/fixture/projects"]);
    render(<KeyRepositories keyInfo={key} usage={{ roots: [], hosts: [], repositories: [], profiles: [], warnings: [] }} onChanged={vi.fn()} />); await screen.findByText("No folders added to scan coverage.");
    fireEvent.click(screen.getByRole("button", { name: "Add project folder" }));
    await waitFor(() => expect(api.setRepositoryRoots).toHaveBeenCalledWith(["/fixture/projects"]));
  });
  it("shows affected repositories before allowing deletion", async () => {
    vi.mocked(api.getKeyUsage).mockResolvedValue({ hosts: ["git-work"], profiles: ["Work"], repositories: ["/fixture/projects/service · origin"], roots: ["/fixture/projects"], warnings: [] });
    const remove = vi.fn(); render(<KeyUsageDialog keyInfo={key} onDelete={remove} onCancel={vi.fn()} />);
    expect(remove).not.toHaveBeenCalled(); await screen.findByText(/service · origin/);
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" })); expect(remove).toHaveBeenCalledOnce();
  });
});
describe("diagnostics, import and agent constraints", () => {
  it("never starts network authentication on mount or a configuration-only check", async () => {
    render(<DiagnosticsPage />); expect(api.diagnoseSsh).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Check configuration" }));
    await waitFor(() => expect(api.diagnoseSsh).toHaveBeenCalledWith("git-work", false, null));
    await screen.findByText(/git@github.com/);
    fireEvent.click(screen.getByRole("button", { name: "Test authentication" }));
    await waitFor(() => expect(api.diagnoseSsh).toHaveBeenLastCalledWith("git-work", true, null));
  });
  it("inspects a chosen key and imports only after review", async () => {
    vi.mocked(open).mockResolvedValue("/fixture/external");
    vi.mocked(api.inspectKeyImport).mockResolvedValue({ key, duplicates: [], public_recovered: true, revision: "source-revision", warning: null });
    vi.mocked(api.importSshKey).mockResolvedValue(key); const imported = vi.fn();
    render(<KeyImportDialog onClose={vi.fn()} onImported={imported} />);
    fireEvent.click(screen.getByRole("button", { name: "Choose private key file" }));
    await screen.findByText(key.fingerprint); expect(api.importSshKey).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("Library filename"), { target: { value: "imported_work" } });
    fireEvent.click(screen.getByRole("button", { name: "Import key" }));
    await waitFor(() => expect(api.importSshKey).toHaveBeenCalledWith("/fixture/external", "imported_work", "source-revision"));
    await waitFor(() => expect(imported).toHaveBeenCalledOnce());
  });
  it("does not allow importing an existing fingerprint twice", async () => {
    vi.mocked(open).mockResolvedValue("/fixture/external");
    vi.mocked(api.inspectKeyImport).mockResolvedValue({ key, duplicates: [key.private_path], public_recovered: false, revision: "r", warning: null });
    render(<KeyImportDialog onClose={vi.fn()} onImported={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Choose private key file" }));
    await screen.findByText(/already in your library/);
    expect((screen.getByRole("button", { name: "Import key" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it("passes the selected lifetime and per-use confirmation policy", () => {
    const confirm = vi.fn(); render(<AgentOptionsDialog keyName="work" onConfirm={confirm} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Keep key available"), { target: { value: "28800" } });
    fireEvent.click(screen.getByLabelText("Require confirmation for every use"));
    fireEvent.click(screen.getByRole("button", { name: "Add with these settings" }));
    expect(confirm).toHaveBeenCalledWith({ lifetime_seconds: 28800, confirm: true });
  });
  it("converts an end-of-workday time into an agent-enforced lifetime", () => {
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date(2026, 8, 30, 9, 0, 0));
      const confirm = vi.fn(); render(<AgentOptionsDialog keyName="work" onConfirm={confirm} onCancel={vi.fn()} />);
      fireEvent.change(screen.getByLabelText("Keep key available"), { target: { value: "end" } });
      fireEvent.click(screen.getByRole("button", { name: "Add with these settings" }));
      expect(confirm).toHaveBeenCalledWith({ lifetime_seconds: 9 * 3600, confirm: false });
    } finally { vi.useRealTimers(); }
  });
  it("keeps an explicit remote port in the diagnostic request", async () => {
    useStore.setState({ diagnosticAlias: "alice@git-work", diagnosticPort: 2222 });
    render(<DiagnosticsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Check configuration" }));
    await waitFor(() => expect(api.diagnoseSsh).toHaveBeenCalledWith("alice@git-work", false, 2222));
  });

});


describe("access setup from key details", () => {
  const usage = { roots: [], hosts: [], profiles: [], repositories: [], warnings: [] };
  const plan = { repository: "/fixture/project", remote: "origin", before: "https://github.com/team/repo.git", after: "git@git-work:team/repo.git", key_path: key.private_path, fingerprint: key.fingerprint, alias: "git-work", revision: "git-v1", ssh_revision: "ssh-v1", warnings: ["Configuration evidence only"] };
  async function choose() {
    vi.mocked(open).mockResolvedValue("/fixture/project");
    vi.mocked(api.inspectRepository).mockResolvedValue({ roots: ["/fixture/project"], repositories: 1, warnings: [], remotes: [{ repository: "/fixture/project", name: "origin", direction: "fetch/push", url: plan.before, ssh_host: null, ssh_user: null, ssh_port: null, profiles: [], key_paths: [], warnings: [] }] });
    vi.mocked(api.previewRepositoryAccess).mockResolvedValue(plan);
    fireEvent.click(screen.getByRole("button", { name: "Set up repository access" }));
    await screen.findByLabelText("Remote");
  }
  it("only applies the reviewed plan after the explicit Apply action", async () => {
    const changed = vi.fn();
    render(<KeyRepositories keyInfo={key} usage={usage} onChanged={changed} />);
    await choose();
    expect(api.applyRepositoryAccess).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Preview URL change" }));
    await screen.findByText(plan.after);
    expect(api.previewRepositoryAccess).toHaveBeenCalledWith("/fixture/project", "origin", key, "git-work");
    expect(api.applyRepositoryAccess).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Apply URL change" }));
    await screen.findByText("Repository URL updated. Authentication has not been tested.");
    expect(api.applyRepositoryAccess).toHaveBeenCalledWith(plan);
    expect(changed).toHaveBeenCalledOnce();
    expect(api.setRepositoryRoots).not.toHaveBeenCalled();
  });
  it("keeps a rejected stale change visible and cancellation writes nothing", async () => {
    vi.mocked(api.applyRepositoryAccess).mockRejectedValue(new Error("Configuration changed"));
    render(<KeyRepositories keyInfo={key} usage={usage} onChanged={vi.fn()} />);
    await choose();
    fireEvent.click(screen.getByRole("button", { name: "Preview URL change" }));
    await screen.findByText(plan.after);
    fireEvent.click(screen.getByRole("button", { name: "Apply URL change" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText(plan.after)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close setup" }));
    expect(api.applyRepositoryAccess).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(plan.after)).toBeNull();
  });
  it("offers only profiles referencing the selected key", async () => {
    vi.mocked(api.listGitProfiles).mockResolvedValue({ profiles: [{ ...profile, key_path: "/other/key" }], revision: "v" });
    render(<KeyRepositories keyInfo={key} usage={usage} onChanged={vi.fn()} />);
    vi.mocked(open).mockResolvedValue("/fixture/project");
    vi.mocked(api.inspectRepository).mockResolvedValue({ roots: ["/fixture/project"], repositories: 1, remotes: [], warnings: [] });
    fireEvent.click(screen.getByRole("button", { name: "Set up repository access" }));
    await screen.findByText(/Create an alias for this key/);
    expect(screen.queryByRole("button", { name: "Preview URL change" })).toBeNull();
  });
});
