// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { KeyRotation } from "./KeyRotation";
import * as api from "../../lib/tauri";
vi.mock("../../lib/tauri", () => ({ listSshKeys: vi.fn(), previewKeyRotation: vi.fn(), applyKeyRotation: vi.fn() }));
const oldKey: api.SshKeyInfo = { name: "old", private_path: "/ssh/old", public_path: "/ssh/old.pub", fingerprint: "SHA256:old", algorithm: "ed25519", bits: 256, comment: "", has_passphrase: true, created_at: null };
const newKey = { ...oldKey, name: "new", private_path: "/ssh/new", fingerprint: "SHA256:new" };
const plan: api.KeyRotationPlan = { old_path: oldKey.private_path, new_path: newKey.private_path, old_fingerprint: oldKey.fingerprint, new_fingerprint: newKey.fingerprint, revisions: { "/ssh/config": "v1" }, warnings: ["No network checks"], targets: [
  { id: "host:1", kind: "host", label: "production", source: "/ssh/config", line: 3, before: "IdentityFile /ssh/old", after: "IdentityFile /ssh/new", blocked: null },
  { id: "profile:work", kind: "profile", label: "Work", source: "/ssh/config", line: 0, before: "/ssh/old", after: "/ssh/new", blocked: null },
  { id: "host:2", kind: "host", label: "Match all", source: "/ssh/config", line: 9, before: "IdentityFile /ssh/old", after: "IdentityFile /ssh/new", blocked: "Manual review required" },
] };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(api.listSshKeys).mockResolvedValue([oldKey, newKey, { ...oldKey, name: "copy", private_path: "/ssh/copy" }]); vi.mocked(api.previewKeyRotation).mockResolvedValue(plan); });
afterEach(cleanup);
async function start() {
  fireEvent.click(screen.getByRole("button", { name: "Replace key references" }));
  await screen.findByRole("option", { name: /new · SHA256:new/ });
  fireEvent.change(screen.getByLabelText("Replacement key"), { target: { value: newKey.private_path } });
  fireEvent.click(screen.getByRole("button", { name: "Find references" }));
  await screen.findByText("SSH host: production");
}
it("requires explicit selection, preview and registration before applying only chosen links", async () => {
  const changed=vi.fn(); render(<KeyRotation keyInfo={oldKey} onChanged={changed} onBusy={vi.fn()} />);
  await start();
  expect(api.previewKeyRotation).toHaveBeenCalledWith(oldKey,newKey);
  const boxes=screen.getAllByRole("checkbox") as HTMLInputElement[];
  expect(boxes.every(b=>!b.checked)).toBe(true); expect(boxes[2].disabled).toBe(true);
  fireEvent.click(boxes[0]);
  fireEvent.click(screen.getByRole("button", { name: "Review 1 selected references" }));
  expect(screen.getByText("IdentityFile /ssh/new")).toBeTruthy();
  const apply=screen.getByRole("button",{name:"Apply replacement"}) as HTMLButtonElement;
  expect(apply.disabled).toBe(true); expect(api.applyKeyRotation).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox",{name:/I have registered/})); fireEvent.click(apply);
  await screen.findByText("Selected references now use the new key.");
  expect(api.applyKeyRotation).toHaveBeenCalledWith(plan,["host:1"]); expect(changed).toHaveBeenCalledOnce();
});
it("excludes copies of the old identity and cancellation does not write",async()=>{
  render(<KeyRotation keyInfo={oldKey} onChanged={vi.fn()} onBusy={vi.fn()} />);
  fireEvent.click(screen.getByRole("button",{name:"Replace key references"}));
  await screen.findByRole("option",{name:/new ·/});
  expect(screen.queryByRole("option",{name:/copy ·/})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(screen.queryByRole("dialog")).toBeNull(); expect(api.applyKeyRotation).not.toHaveBeenCalled();
});
it("keeps stale failures visible and refreshing clears selection and consent",async()=>{
  vi.mocked(api.applyKeyRotation).mockRejectedValue(new Error("SSH config changed"));
  render(<KeyRotation keyInfo={oldKey} onChanged={vi.fn()} onBusy={vi.fn()} />);await start();
  fireEvent.click(screen.getAllByRole("checkbox")[0]);fireEvent.click(screen.getByRole("button",{name:"Review 1 selected references"}));
  fireEvent.click(screen.getByRole("checkbox",{name:/I have registered/}));fireEvent.click(screen.getByRole("button",{name:"Apply replacement"}));
  await screen.findByRole("alert");fireEvent.click(screen.getByRole("button",{name:"Refresh replacement plan"}));
  await waitFor(()=>expect(screen.getAllByRole("checkbox").length).toBe(3));
  expect((screen.getByRole("button",{name:"Review 0 selected references"}) as HTMLButtonElement).disabled).toBe(true);
});
