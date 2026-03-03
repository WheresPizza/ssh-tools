import { describe, it, expect } from "vitest";
import { filterHosts } from "./ssh-host-utils";
import type { SshHost } from "./tauri";

function makeHost(o: Partial<SshHost> = {}): SshHost {
  return {
    alias: "myserver", hostname: "192.168.1.1", user: "alice", port: 22,
    identity_file: [], proxy_jump: null, forward_agent: null,
    server_alive_interval: null, extra_fields: [], line_start: 0, line_end: 2,
    ...o,
  };
}

describe("filterHosts", () => {
  const hosts = [
    makeHost({ alias: "webserver", hostname: "example.com", user: "deploy" }),
    makeHost({ alias: "devbox",    hostname: "dev.internal", user: "alice" }),
    makeHost({ alias: "prod-db",   hostname: "db.prod.io",   user: "root" }),
  ];

  it("empty query → returns all hosts unchanged", () => {
    expect(filterHosts(hosts, "")).toEqual(hosts);
  });

  it("whitespace-only query → returns all hosts unchanged", () => {
    expect(filterHosts(hosts, "   ")).toEqual(hosts);
  });

  it("matches alias (case-insensitive)", () => {
    const result = filterHosts(hosts, "WEBSERVER");
    expect(result).toHaveLength(1);
    expect(result[0].alias).toBe("webserver");
  });

  it("matches hostname (case-insensitive)", () => {
    const result = filterHosts(hosts, "EXAMPLE.COM");
    expect(result).toHaveLength(1);
    expect(result[0].hostname).toBe("example.com");
  });

  it("matches user (case-insensitive)", () => {
    const result = filterHosts(hosts, "DEPLOY");
    expect(result).toHaveLength(1);
    expect(result[0].user).toBe("deploy");
  });

  it("partial match works", () => {
    const result = filterHosts(hosts, "dev");
    expect(result).toHaveLength(1); // "devbox" matches via alias (and also hostname "dev.internal")
    expect(result[0].alias).toBe("devbox");
  });

  it("no match → returns empty array", () => {
    expect(filterHosts(hosts, "zzznomatch")).toEqual([]);
  });

  it("hostname: null host is handled without crash", () => {
    const nullHostname = [makeHost({ alias: "nohost", hostname: null, user: "bob" })];
    expect(() => filterHosts(nullHostname, "anything")).not.toThrow();
    expect(filterHosts(nullHostname, "nohost")).toHaveLength(1);
  });

  it("user: null host is handled without crash", () => {
    const nullUser = [makeHost({ alias: "nouser", hostname: "some.host", user: null })];
    expect(() => filterHosts(nullUser, "anything")).not.toThrow();
    expect(filterHosts(nullUser, "nouser")).toHaveLength(1);
  });
});
