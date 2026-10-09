// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TerminalOperations } from "./TerminalOperations";
import { useOperations } from "../../lib/operations";
import * as api from "../../lib/tauri";
vi.mock("../../lib/tauri", () => ({ listTerminalOperations: vi.fn(), dismissTerminalOperation: vi.fn() }));
const pending: api.TerminalOperation = { id: "op-fixture", kind: "agent_add", target: "/fixture/key", state: "awaiting_input", detail: "Enter passphrase in the terminal", fingerprint: "SHA256:fixture", exit_code: null, started_at: 0 };
beforeEach(() => { vi.resetAllMocks(); useOperations.setState({ operations: [] }); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("keeps waiting until a verified completion arrives and refreshes agent state", async () => {
  vi.useFakeTimers();
  vi.mocked(api.listTerminalOperations).mockResolvedValue([pending]);
  const changed = vi.fn(); window.addEventListener("ssh-agent-changed", changed);
  render(<TerminalOperations />);
  await act(async () => {});
  expect(screen.getByText(/awaiting input/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Dismiss result" })).toBeNull();
  expect(changed).not.toHaveBeenCalled();
  vi.mocked(api.listTerminalOperations).mockResolvedValue([{ ...pending, state: "completed", exit_code: 0, detail: "Fingerprint verified" }]);
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.getByText("Fingerprint verified")).toBeTruthy();
  expect(changed).toHaveBeenCalledOnce();
  await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
  expect(changed).toHaveBeenCalledOnce();
  window.removeEventListener("ssh-agent-changed", changed);
});
it("recovers a failed operation on mount and only removes it after explicit dismissal", async () => {
  vi.mocked(api.listTerminalOperations).mockResolvedValue([{ ...pending, state: "failed", detail: "Passphrase input failed" }]);
  vi.mocked(api.dismissTerminalOperation).mockResolvedValue(undefined);
  render(<TerminalOperations />);
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("Passphrase input failed"));
  vi.mocked(api.listTerminalOperations).mockResolvedValue([]);
  fireEvent.click(screen.getByRole("button", { name: "Dismiss result" }));
  await act(async () => {});
  expect(api.dismissTerminalOperation).toHaveBeenCalledWith(pending.id);
  expect(screen.queryByRole("alert")).toBeNull();
});
