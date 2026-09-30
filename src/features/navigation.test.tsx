// @vitest-environment jsdom
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import App from "../App";
import { SettingsPage } from "./settings/SettingsPage";
import { Sidebar } from "../components/layout/Sidebar";
import { useStore } from "../stores";
import * as api from "../lib/tauri";
vi.mock("../lib/tauri", () => ({
  getDetectedTerminal: vi.fn(), setPreferredTerminal: vi.fn(), listBackups: vi.fn(),
  getWorkspace: vi.fn(), getSshConfig: vi.fn(), launchSshConnection: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
beforeEach(() => {
  vi.clearAllMocks();
  useStore.persist.setOptions({ storage: { getItem: () => null, setItem: () => {}, removeItem: () => {} } });
  useStore.setState({ activeTab: "ssh-config", hosts: [], toast: null });
  vi.mocked(api.getDetectedTerminal).mockResolvedValue([
    { name: "Terminal", path: "/System/Applications/Utilities/Terminal.app", is_preferred: true },
    { name: "iTerm2", path: "/Applications/iTerm.app", is_preferred: false },
  ]);
  vi.mocked(api.listBackups).mockResolvedValue([]);
  vi.mocked(api.getSshConfig).mockResolvedValue([]);
  vi.mocked(api.getWorkspace).mockResolvedValue({ ssh_dir: "/fixture/ssh", isolated: true });
});
afterEach(cleanup);
it("groups entity navigation and replaces Launcher with Settings", () => {
  render(<Sidebar />);
  const nav = screen.getByRole("navigation", { name: "Workspace" });
  expect(within(nav).queryByRole("button", { name: "Launcher" })).toBeNull();
  expect(within(nav).getByRole("button", { name: "SSH Hosts" }).getAttribute("aria-current")).toBe("page");
  fireEvent.click(within(nav).getByRole("button", { name: "Settings" }));
  expect(useStore.getState().activeTab).toBe("settings");
  expect(screen.getByText("SECURITY")).toBeTruthy();
});
it("owns terminal preferences and recovery without duplicating host connections", async () => {
  render(<SettingsPage />);
  fireEvent.click(await screen.findByRole("button", { name: "iTerm2" }));
  await waitFor(() => expect(api.setPreferredTerminal).toHaveBeenCalledWith("iTerm2"));
  expect(api.getSshConfig).not.toHaveBeenCalled();
  expect(api.launchSshConnection).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
  expect(screen.getAllByText("Backups and recovery")).toHaveLength(1);
  await waitFor(() => expect(api.getDetectedTerminal).toHaveBeenCalledTimes(2));
});
it("migrates a saved Launcher tab to Hosts and routes recovery to Settings", async () => {
  useStore.persist.setOptions({ storage: {
    getItem: () => ({ state: { activeTab: "launcher", connectionHistory: {} }, version: 0 }),
    setItem: () => {}, removeItem: () => {},
  } });
  render(<App />);
  expect(await screen.findByRole("heading", { name: "SSH Hosts" })).toBeTruthy();
  expect(useStore.getState().activeTab).toBe("ssh-config");
  expect(screen.queryByText("Backups and recovery")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "File recovery in Settings" }));
  expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
});
