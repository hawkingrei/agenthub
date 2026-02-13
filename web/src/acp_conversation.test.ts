import { describe, expect, it } from "vitest";
import { deriveToolCallOpenState } from "./components/acp_conversation";

describe("deriveToolCallOpenState", () => {
  it("keeps details open while tool call is live", () => {
    expect(deriveToolCallOpenState(false, false, true)).toBe(true);
    expect(deriveToolCallOpenState(true, true, true)).toBe(true);
  });

  it("auto-collapses when a live tool call transitions to finished", () => {
    expect(deriveToolCallOpenState(true, true, false)).toBe(false);
  });

  it("preserves manual toggle state for non-live tool calls", () => {
    expect(deriveToolCallOpenState(true, false, false)).toBe(true);
    expect(deriveToolCallOpenState(false, false, false)).toBe(false);
  });
});
