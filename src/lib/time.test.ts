import { describe, it, expect, vi, afterEach } from "vitest";
import { formatTimeAgo } from "./time";

describe("formatTimeAgo", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  function atNow(offsetMs: number) {
    const now = 1_700_000_000_000;
    vi.setSystemTime(now);
    return now - offsetMs;
  }

  it("returns 'just now' for less than 60 seconds ago", () => {
    const ts = atNow(30_000);
    expect(formatTimeAgo(ts)).toBe("just now");
  });

  it("returns 'just now' for 0 seconds ago", () => {
    const ts = atNow(0);
    expect(formatTimeAgo(ts)).toBe("just now");
  });

  it("returns '1 minute ago' for exactly 60 seconds", () => {
    const ts = atNow(60_000);
    expect(formatTimeAgo(ts)).toBe("1 minute ago");
  });

  it("returns '5 minutes ago' for 5 minutes", () => {
    const ts = atNow(5 * 60_000);
    expect(formatTimeAgo(ts)).toBe("5 minutes ago");
  });

  it("returns '1 hour ago' for exactly 60 minutes", () => {
    const ts = atNow(60 * 60_000);
    expect(formatTimeAgo(ts)).toBe("1 hour ago");
  });

  it("returns '2 hours ago' for 2 hours", () => {
    const ts = atNow(2 * 3600_000);
    expect(formatTimeAgo(ts)).toBe("2 hours ago");
  });

  it("returns 'yesterday' for exactly 24 hours ago", () => {
    const ts = atNow(24 * 3600_000);
    expect(formatTimeAgo(ts)).toBe("yesterday");
  });

  it("returns '3 days ago' for 3 days", () => {
    const ts = atNow(3 * 24 * 3600_000);
    expect(formatTimeAgo(ts)).toBe("3 days ago");
  });

  it("returns a formatted date for 30+ days ago", () => {
    const ts = atNow(35 * 24 * 3600_000);
    const result = formatTimeAgo(ts);
    // Should be a short date like "Oct 1" — just verify it's not a relative string
    expect(result).not.toContain("ago");
    expect(result).not.toBe("yesterday");
    expect(result.length).toBeGreaterThan(0);
  });
});
