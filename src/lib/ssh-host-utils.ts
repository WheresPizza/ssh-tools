import type { SshHost } from "./tauri";

export function filterHosts(hosts: SshHost[], query: string): SshHost[] {
  if (query.trim() === "") return hosts;
  const q = query.toLowerCase();
  return hosts.filter(
    (h) =>
      h.alias.toLowerCase().includes(q) ||
      (h.hostname ?? "").toLowerCase().includes(q) ||
      (h.user ?? "").toLowerCase().includes(q)
  );
}
