import { describe, it, expect, vi } from "vitest";
const mockListen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));
import { subscribe } from "./events";
describe("Tauri event subscriptions", () => {
  it("cleans up even if the component unmounts before listen resolves", async () => {
    let resolve!: (fn: () => void) => void;
    mockListen.mockReturnValue(new Promise<() => void>(r => { resolve = r; }));
    const cleanup = subscribe("ssh-config-changed", () => {});
    cleanup();
    const unlisten = vi.fn();
    resolve(unlisten);
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
