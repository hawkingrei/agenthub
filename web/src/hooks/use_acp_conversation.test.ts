import { describe, expect, it } from "vitest";
import { ConversationItem } from "../conversation";
import {
  buildConversationTailKey,
  buildVirtualConversationSlice,
  deriveConversationStickToBottom,
  deriveConversationJumpState,
  estimateToolCallJumpTop,
  estimateTailPayloadSize,
  findToolCallNodeById,
  findConversationToolCallIndex,
  nextConversationViewport,
  normalizeConversationAvgHeightEstimate,
  restoreConversationScrollTop,
  shouldAutoLoadConversationHistory,
  shouldLoadOlderFromMeta,
  shouldUseConversationVirtualization,
} from "./use_acp_conversation";

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

  it("falls back to default item height for invalid estimates", () => {
    const items = makeItems(300);
    const slice = buildVirtualConversationSlice(items, 0, -80, 0, 0);
    expect(slice.offset).toBe(0);
    expect(slice.items.length).toBeGreaterThan(0);
    expect(slice.items.length).toBeLessThanOrEqual(300);
    expect(slice.topSpacer).toBe(0);
  });
});

describe("buildConversationTailKey", () => {
  it("handles empty conversations", () => {
    expect(buildConversationTailKey([])).toBe("empty");
  });

  it("builds key for plain message item", () => {
    const key = buildConversationTailKey([
      {
        kind: "agent_message",
        text: "hello",
        event_id: 42,
      },
    ]);
    expect(key).toBe("agent_message:42:5");
  });

  it("builds key for tool call including payload lengths", () => {
    const key = buildConversationTailKey([
      {
        kind: "tool_call",
        id: "call-1",
        title: "search",
        content: "abc",
        terminal_output: "ok",
        raw_input: { q: "hello" },
        raw_output: { done: true },
        event_id: 7,
      },
    ]);
    expect(key).toContain("tool_call:7:3:2:");
  });

  it("uses lightweight payload size estimation for large payloads", () => {
    const huge = {
      items: Array.from({ length: 200 }, (_, idx) => ({
        id: idx,
        text: `payload-${idx}`.repeat(10),
      })),
    };
    const key = buildConversationTailKey([
      {
        kind: "tool_call",
        id: "call-2",
        title: "search",
        raw_input: huge,
        raw_output: huge,
        event_id: 8,
      },
    ]);
    expect(key).toContain("tool_call:8:");
    expect(key.length).toBeLessThan(120);
  });

  it("builds key for grouped tool calls using trailing call payload lengths", () => {
    const key = buildConversationTailKey([
      {
        kind: "tool_call_group",
        calls: [
          {
            kind: "tool_call",
            id: "call-1",
            title: "Search",
            content: "first",
            event_id: 9,
          },
          {
            kind: "tool_call",
            id: "call-2",
            title: "Read",
            content: "tail-content",
            terminal_output: "ok",
            raw_input: { path: "README.md" },
            raw_output: { done: true },
            event_id: 10,
          },
        ],
        event_id: 10,
      },
    ]);
    expect(key).toContain("tool_call_group:10:count:2:12:2:");
  });
});

describe("findConversationToolCallIndex", () => {
  it("returns matching tool call index", () => {
    const items: ConversationItem[] = [
      { kind: "agent_message", text: "a", event_id: 1 },
      { kind: "tool_call", id: "call-1", title: "Read", event_id: 2 },
      { kind: "tool_call", id: "call-2", title: "Write", event_id: 3 },
    ];
    expect(findConversationToolCallIndex(items, "call-2")).toBe(2);
  });

  it("returns -1 when id is empty or missing", () => {
    const items: ConversationItem[] = [
      { kind: "agent_message", text: "a", event_id: 1 },
      { kind: "tool_call", id: "call-1", title: "Read", event_id: 2 },
    ];
    expect(findConversationToolCallIndex(items, "")).toBe(-1);
    expect(findConversationToolCallIndex(items, "missing")).toBe(-1);
  });

  it("returns grouped item index when call is nested in tool_call_group", () => {
    const items: ConversationItem[] = [
      { kind: "agent_message", text: "a", event_id: 1 },
      {
        kind: "tool_call_group",
        calls: [
          { kind: "tool_call", id: "call-1", title: "Read", event_id: 2 },
          { kind: "tool_call", id: "call-2", title: "Write", event_id: 3 },
        ],
        event_id: 3,
      },
      { kind: "agent_message", text: "b", event_id: 4 },
    ];
    expect(findConversationToolCallIndex(items, "call-2")).toBe(1);
  });
});

describe("tool call jump helpers", () => {
  it("estimates scroll top with context lines and minimum row height", () => {
    expect(estimateToolCallJumpTop(10, 48)).toBe(288);
    expect(estimateToolCallJumpTop(2, 18)).toBe(0);
  });

  it("finds matching tool call node from data attribute", () => {
    const target = {
      getAttribute: (name: string) => (name === "data-tool-call-id" ? "call-2" : null),
    };
    const container = {
      querySelectorAll: () =>
        [
          {
            getAttribute: (name: string) =>
              name === "data-tool-call-id" ? "call-1" : null,
          },
          target,
        ] as unknown as NodeListOf<Element>,
    } as unknown as ParentNode;
    expect(findToolCallNodeById(container, "call-2")).toBe(target);
    expect(findToolCallNodeById(container, "missing")).toBeNull();
  });
});

describe("estimateTailPayloadSize", () => {
  it("handles primitives and nested structures deterministically", () => {
    expect(estimateTailPayloadSize(null)).toBe(0);
    expect(estimateTailPayloadSize("hello")).toBe(5);
    expect(estimateTailPayloadSize(12)).toBe(8);
    expect(estimateTailPayloadSize(true)).toBe(8);
    expect(
      estimateTailPayloadSize({
        q: "agenthub",
        tags: ["a", "b", "c"],
        meta: { page: 1, ok: true },
      })
    ).toBeGreaterThan(20);
  });
});

describe("shouldLoadOlderFromMeta", () => {
  const meta = {
    "agent-1:latest": {
      oldestId: 1,
      hasMore: true,
      loading: false,
      loaded: true,
    },
  };

  it("returns false without active agent", () => {
    expect(shouldLoadOlderFromMeta(null, null, meta)).toBe(false);
  });

  it("returns false when meta is missing or not eligible", () => {
    expect(shouldLoadOlderFromMeta("agent-x", null, meta)).toBe(false);
    expect(
      shouldLoadOlderFromMeta("agent-1", null, {
        "agent-1:latest": { ...meta["agent-1:latest"], loading: true },
      })
    ).toBe(false);
    expect(
      shouldLoadOlderFromMeta("agent-1", null, {
        "agent-1:latest": { ...meta["agent-1:latest"], hasMore: false },
      })
    ).toBe(false);
    expect(
      shouldLoadOlderFromMeta("agent-1", null, {
        "agent-1:latest": { ...meta["agent-1:latest"], oldestId: null },
      })
    ).toBe(false);
  });

  it("returns true when metadata indicates older events are available", () => {
    expect(shouldLoadOlderFromMeta("agent-1", null, meta)).toBe(true);
  });
});

describe("conversation helper decisions", () => {
  it("derives virtualization eligibility", () => {
    expect(shouldUseConversationVirtualization(true, 500)).toBe(false);
    expect(shouldUseConversationVirtualization(false, 50)).toBe(false);
    expect(shouldUseConversationVirtualization(false, 180)).toBe(true);
  });

  it("computes next viewport state and reuses previous object when unchanged", () => {
    const prev = { top: 10, height: 400 };
    expect(nextConversationViewport(prev, 10, 400)).toBe(prev);
    expect(nextConversationViewport(prev, 20, 420)).toEqual({ top: 20, height: 420 });
  });

  it("derives auto-load eligibility for short conversation windows", () => {
    expect(shouldAutoLoadConversationHistory("debug", "agent-1", true, 2)).toBe(false);
    expect(shouldAutoLoadConversationHistory("conversation", null, true, 2)).toBe(false);
    expect(
      shouldAutoLoadConversationHistory("conversation", "agent-1", false, 2)
    ).toBe(false);
    expect(
      shouldAutoLoadConversationHistory("conversation", "agent-1", true, 20)
    ).toBe(false);
    expect(
      shouldAutoLoadConversationHistory("conversation", "agent-1", true, 4)
    ).toBe(true);
  });

  it("normalizes average height estimates with bounds and jitter guard", () => {
    expect(normalizeConversationAvgHeightEstimate(48, 0, 4)).toBe(48);
    expect(normalizeConversationAvgHeightEstimate(48, 96, 2)).toBe(48);
    expect(normalizeConversationAvgHeightEstimate(48, 960, 4)).toBe(220);
    expect(normalizeConversationAvgHeightEstimate(48, 8, 10)).toBe(24);
  });

  it("restores scroll top within available scroll range", () => {
    expect(restoreConversationScrollTop(400, 800, 300)).toBe(400);
    expect(restoreConversationScrollTop(900, 800, 300)).toBe(500);
  });

  it("derives jump/badge visibility from tab and stick state", () => {
    expect(deriveConversationJumpState("debug", false, 3)).toEqual({
      showConversationJump: false,
      showConversationBadge: false,
    });
    expect(deriveConversationJumpState("conversation", true, 3)).toEqual({
      showConversationJump: false,
      showConversationBadge: false,
    });
    expect(deriveConversationJumpState("conversation", false, 0)).toEqual({
      showConversationJump: true,
      showConversationBadge: false,
    });
    expect(deriveConversationJumpState("conversation", false, 2)).toEqual({
      showConversationJump: true,
      showConversationBadge: true,
    });
  });

  it("keeps stick enabled for passive viewport/content changes while sticky", () => {
    expect(
      deriveConversationStickToBottom(
        1200,
        700,
        300,
        true,
        700
      )
    ).toBe(true);
  });

  it("keeps sticky mode when previous scroll top is unavailable", () => {
    expect(
      deriveConversationStickToBottom(
        1600,
        900,
        300,
        true,
        null
      )
    ).toBe(true);
  });

  it("does not detach stick for tiny upward movement", () => {
    expect(
      deriveConversationStickToBottom(
        1600,
        900,
        300,
        true,
        920
      )
    ).toBe(true);
  });

  it("keeps sticky mode on downward movement while not near bottom", () => {
    expect(
      deriveConversationStickToBottom(
        1600,
        950,
        300,
        true,
        900
      )
    ).toBe(true);
  });

  it("detaches stick when user scrolls up beyond threshold", () => {
    expect(
      deriveConversationStickToBottom(
        1600,
        860,
        300,
        true,
        920
      )
    ).toBe(false);
  });

  it("keeps non-sticky state until near-bottom", () => {
    expect(
      deriveConversationStickToBottom(
        1800,
        1200,
        300,
        false,
        1220
      )
    ).toBe(false);
    expect(
      deriveConversationStickToBottom(
        1800,
        1385,
        300,
        false,
        1220
      )
    ).toBe(true);
  });
});
