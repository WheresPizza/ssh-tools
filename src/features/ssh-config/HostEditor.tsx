import { useEffect, useState } from "react";
import { listSshKeys, type SshKeyInfo } from "../../lib/tauri";
import type { SshHost } from "../../lib/tauri";

interface HostEditorProps {
  host: SshHost | null;
  isNew?: boolean;
  onSave: (host: SshHost) => Promise<void>;
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
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [saving, setSaving] = useState(false);
  useEffect(() => { listSshKeys().then(setKeys).catch(() => setKeys([])); }, []);
  const [form, setForm] = useState<SshHost>(host ?? EMPTY_HOST);
  const [identityInput, setIdentityInput] = useState(
    (host?.identity_file ?? []).join("\n")
  );

  const set = (key: keyof SshHost, value: unknown) =>
    setForm((f) => ({ ...f, [key]: value }));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const saved: SshHost = {
      ...form,
      identity_file: identityInput
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
    };
    if (saving) return;
    setSaving(true);
    try { await onSave(saved); } finally { setSaving(false); }
  };

  return (
    <div>
      <div className="flex items-center gap-3 mb-5">
        <button
          onClick={onCancel}
          className="btn text-muted-foreground hover:text-foreground"
        >
          ← Back
        </button>
        <h2 className="text-lg font-semibold">
          {isNew ? "Add SSH Host" : host ? `Edit: ${host.alias}` : "Add SSH Host"}
        </h2>
      </div>

      {host?.source_path && <p className="mb-4 text-xs text-muted-foreground">File: {host.source_path}</p>}
      <form onSubmit={handleSubmit} className="max-w-lg space-y-4">
        <Field label="Alias" required>
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
        <Field label="Choose an SSH key">
          <select className="input-field" value="" onChange={e => {
            const value = e.target.value;
            if (value) setIdentityInput(previous => [...new Set([...previous.split("\n").filter(Boolean), value])].join("\n"));
          }}>
            <option value="">Select a key to add…</option>
            {keys.filter(key => !key.error).map(key => <option key={key.private_path} value={JSON.stringify(key.private_path)}>{key.name} — {key.algorithm}</option>)}
          </select>
        </Field>
        <label className="flex gap-2 text-sm items-start">
          <input type="checkbox" checked={form.extra_fields.some(([k,v]) => k.toLowerCase() === "identitiesonly" && v.toLowerCase() === "yes")} onChange={e => set("extra_fields", [...form.extra_fields.filter(([k]) => k.toLowerCase() !== "identitiesonly"), ["IdentitiesOnly", e.target.checked ? "yes" : "no"]])} />
          Use only the configured identities
        </label>
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
            disabled={saving}
            className="btn btn-primary"
          >
            {saving ? "Saving…" : isNew ? "Add Host" : host ? "Save Changes" : "Add Host"}
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="btn"
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
