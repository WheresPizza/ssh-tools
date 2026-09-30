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

export function isConnectableAlias(alias: string): boolean {
  return alias.length > 0 && !alias.startsWith("-") && [...alias].every(c => /[\p{L}\p{N}]/u.test(c) || "._-:@[]%".includes(c));
}

export function mergeVisibleHostOrder(hosts: SshHost[], reordered: SshHost[]): SshHost[] {
  const visible = new Set(reordered.map(hostId));
  let index = 0;
  return hosts.map(host => visible.has(hostId(host)) ? reordered[index++] : host);
}

export function hostId(host: SshHost): string { return `${host.source_path ?? ""}:${host.line_start}:${host.alias}`; }
