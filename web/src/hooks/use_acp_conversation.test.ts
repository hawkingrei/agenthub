import { describe, expect, it } from "vitest";
import { ConversationItem } from "../conversation";
import { buildVirtualConversationSlice } from "./use_acp_conversation";

function makeItems(count: number): ConversationItem[] {
  return Array.from({ length: count }, (_, idx) => ({
    kind: "agent_message",
    text: `message-${idx}`,
    event_id: idx + 1,
  }));
}

describe("buildVirtualConversationSlice", () => {
  it("returns full list when viewport covers the beginning", () => {
    const items = makeItems(20);
    const slice = buildVirtualConversationSlice(items, 0, 0, 600, 30);
    expect(slice.items.length).toBe(20);
    expect(slice.offset).toBe(0);
    expect(slice.topSpacer).toBe(0);
    expect(slice.bottomSpacer).toBe(0);
  });

  it("returns a window with top and bottom spacers for long lists", () => {
    const items = makeItems(1200);
    const slice = buildVirtualConversationSlice(items, 200, 6000, 500, 40, 10);
    expect(slice.items.length).toBeGreaterThan(0);
    expect(slice.items.length).toBeLessThan(1200);
    expect(slice.offset).toBeGreaterThan(200);
    expect(slice.topSpacer).toBeGreaterThan(0);
    expect(slice.bottomSpacer).toBeGreaterThan(0);
  });

  it("clamps overshot viewport top to avoid empty slices", () => {
    const items = makeItems(220);
    const slice = buildVirtualConversationSlice(items, 0, 999_999, 420, 36, 8);
    expect(slice.items.length).toBeGreaterThan(0);
    expect(slice.offset).toBeGreaterThanOrEqual(0);
    expect(slice.offset).toBeLessThan(items.length);
    expect(slice.bottomSpacer).toBeGreaterThanOrEqual(0);
  });
});
