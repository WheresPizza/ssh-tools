// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { Toast } from "./Toast";
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("keeps errors until explicit dismissal", () => {
  vi.useFakeTimers(); const close = vi.fn();
  render(<Toast message="Operation failed" type="error" onClose={close} />);
  vi.advanceTimersByTime(30000);
  expect(close).not.toHaveBeenCalled();
});
