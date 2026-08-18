import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// Unmount between tests so a component's cleanup path (App aborts its in-flight probe) actually
// runs, and drop any stubbed global so one test's fake `fetch` cannot leak into the next.
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
