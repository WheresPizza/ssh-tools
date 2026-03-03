import { useState } from "react";
import type { SshHost } from "../../lib/tauri";

interface HostEditorProps {
  host: SshHost | null;
  isNew?: boolean;
  onSave: (host: SshHost) => void;
  onCancel: () => void;
}

const EMPTY_HOST: SshHost = {
  alias: "",
  hostname: null,
  user: null,
  port: null,
  identity_file: [],
  proxy_jump: null,
  forward_agent: null,
  server_alive_interval: null,
  extra_fields: [],
  line_start: 0,
  line_end: 0,
};

export function HostEditor({ host, isNew = false, onSave, onCancel }: HostEditorProps) {
  const [form, setForm] = useState<SshHost>(host ?? EMPTY_HOST);
  const [identityInput, setIdentityInput] = useState(
    (host?.identity_file ?? []).join("\n")
  );

  const set = (key: keyof SshHost, value: unknown) =>
    setForm((f) => ({ ...f, [key]: value }));

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const saved: SshHost = {
      ...form,
      identity_file: identityInput
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
    };
    onSave(saved);
  };

  return (
    <div>
      <div className="flex items-center gap-3 mb-5">
        <button
          onClick={onCancel}
          className="text-muted-foreground hover:text-foreground text-sm transition-colors"
        >
          ← Back
        </button>
        <h2 className="text-lg font-semibold">
          {isNew ? "Add SSH Host" : host ? `Edit: ${host.alias}` : "Add SSH Host"}
        </h2>
      </div>

      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        <Field label="Alias *" required>
          <input
            required
            value={form.alias}
            onChange={(e) => set("alias", e.target.value)}
            placeholder="my-server"
            className="input-field"
          />
        </Field>
        <Field label="Hostname">
          <input
            value={form.hostname ?? ""}
            onChange={(e) => set("hostname", e.target.value || null)}
            placeholder="192.168.1.1 or example.com"
            className="input-field"
          />
        </Field>
        <div className="grid grid-cols-2 gap-4">
          <Field label="User">
            <input
              value={form.user ?? ""}
              onChange={(e) => set("user", e.target.value || null)}
              placeholder="ubuntu"
              className="input-field"
            />
          </Field>
          <Field label="Port">
            <input
              type="number"
              min={1}
              max={65535}
              value={form.port ?? ""}
              onChange={(e) => set("port", e.target.value ? parseInt(e.target.value) : null)}
              placeholder="22"
              className="input-field"
            />
          </Field>
        </div>
        <Field label="Identity File(s)" hint="One path per line">
          <textarea
            value={identityInput}
            onChange={(e) => setIdentityInput(e.target.value)}
            placeholder="~/.ssh/id_ed25519"
            rows={3}
            className="input-field resize-none font-mono text-xs"
          />
        </Field>
        <Field label="ProxyJump">
          <input
            value={form.proxy_jump ?? ""}
            onChange={(e) => set("proxy_jump", e.target.value || null)}
            placeholder="bastion-host"
            className="input-field"
          />
        </Field>
        <div className="grid grid-cols-2 gap-4">
          <Field label="Forward Agent">
            <select
              value={form.forward_agent === null ? "" : form.forward_agent ? "yes" : "no"}
              onChange={(e) =>
                set("forward_agent", e.target.value === "" ? null : e.target.value === "yes")
              }
              className="input-field"
            >
              <option value="">–</option>
              <option value="yes">yes</option>
              <option value="no">no</option>
            </select>
          </Field>
          <Field label="ServerAliveInterval">
            <input
              type="number"
              min={0}
              value={form.server_alive_interval ?? ""}
              onChange={(e) =>
                set("server_alive_interval", e.target.value ? parseInt(e.target.value) : null)
              }
              placeholder="60"
              className="input-field"
            />
          </Field>
        </div>

        <div className="flex gap-2 pt-2">
          <button
            type="submit"
            className="px-4 py-2 bg-primary text-primary-foreground text-sm rounded-md hover:bg-primary/90 transition-colors font-medium"
          >
            {isNew ? "Add Host" : host ? "Save Changes" : "Add Host"}
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

function Field({
  label,
  required,
  hint,
  children,
}: {
  label: string;
  required?: boolean;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="block text-sm font-medium text-foreground mb-1.5">
        {label}
        {required && <span className="text-destructive ml-0.5">*</span>}
        {hint && <span className="ml-1 text-xs font-normal text-muted-foreground">({hint})</span>}
      </label>
      {children}
    </div>
  );
}
