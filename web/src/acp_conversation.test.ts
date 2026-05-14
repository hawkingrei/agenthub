import { beforeAll, describe, expect, it } from "vitest";
import {
  deriveToolCallOpenState,
  getAcpConversationCacheStats,
  isToolCallEffectivelyLive,
  parseAnsiSegmentsCached,
  renderMarkdownCached,
  resetAcpConversationCaches,
  shouldAutoCollapseConversationItem,
  shouldCollapseToolFoldWhenOutOfView,
} from "./components/acp_conversation";
import { preloadThreadMarkdownAssets } from "./components/thread_rich_text";

beforeAll(async () => {
  await preloadThreadMarkdownAssets();
});

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

describe("shouldCollapseToolFoldWhenOutOfView", () => {
  it("collapses when tool fold is outside visible conversation area", () => {
    expect(shouldCollapseToolFoldWhenOutOfView(false, 0)).toBe(true);
    expect(shouldCollapseToolFoldWhenOutOfView(false, null)).toBe(true);
  });

  it("keeps fold state when still intersecting", () => {
    expect(shouldCollapseToolFoldWhenOutOfView(true, 0.01)).toBe(false);
  });

  it("does not collapse if observer reports non-zero ratio", () => {
    expect(shouldCollapseToolFoldWhenOutOfView(false, 0.2)).toBe(false);
  });
});

describe("isToolCallEffectivelyLive", () => {
  it("treats running tool calls as live when run status is active", () => {
    expect(isToolCallEffectivelyLive("in_progress", "running")).toBe(true);
    expect(isToolCallEffectivelyLive("pending", null)).toBe(true);
  });

  it("treats running-looking tool calls as finished after run is terminal", () => {
    expect(isToolCallEffectivelyLive("in_progress", "completed")).toBe(false);
    expect(isToolCallEffectivelyLive("running", "failed")).toBe(false);
    expect(isToolCallEffectivelyLive("pending", "cancelled")).toBe(false);
  });
});

describe("shouldAutoCollapseConversationItem", () => {
  it("auto-collapses tool bubbles once newer conversation items appear", () => {
    expect(
      shouldAutoCollapseConversationItem(
        { kind: "tool_call", id: "call-1", title: "Read" },
        {
          globalIndex: 3,
          latestVisibleGlobalIndex: 4,
          shouldAutoCollapse: false,
          collapseCutoff: 0,
          isFrozenView: false,
        }
      )
    ).toBe(true);
  });

  it("keeps the newest visible tool bubble expanded until a later item appears", () => {
    expect(
      shouldAutoCollapseConversationItem(
        { kind: "tool_call", id: "call-1", title: "Read" },
        {
          globalIndex: 4,
          latestVisibleGlobalIndex: 4,
          shouldAutoCollapse: false,
          collapseCutoff: 0,
          isFrozenView: false,
        }
      )
    ).toBe(false);
  });

  it("collapses tool bubbles when the surface requests default-collapsed tools", () => {
    expect(
      shouldAutoCollapseConversationItem(
        { kind: "tool_call", id: "call-1", title: "Read" },
        {
          globalIndex: 4,
          latestVisibleGlobalIndex: 4,
          shouldAutoCollapse: false,
          collapseCutoff: 0,
          toolCallsDefaultCollapsed: true,
          isFrozenView: false,
        }
      )
    ).toBe(true);
  });

  it("does not auto-collapse ordinary messages just because newer items exist", () => {
    expect(
      shouldAutoCollapseConversationItem(
        { kind: "agent_message", text: "done" },
        {
          globalIndex: 3,
          latestVisibleGlobalIndex: 4,
          shouldAutoCollapse: false,
          collapseCutoff: 0,
          isFrozenView: false,
        }
      )
    ).toBe(false);
  });
});

describe("conversation render cache", () => {
  it("caches markdown render results by text", () => {
    resetAcpConversationCaches();
    const first = renderMarkdownCached("**cache me**");
    const second = renderMarkdownCached("**cache me**");
    expect(first).toBe(second);
    const stats = getAcpConversationCacheStats();
    expect(stats.markdownMisses).toBe(1);
    expect(stats.markdownHits).toBe(1);
  });

  it("caches ansi parse results by terminal payload", () => {
    resetAcpConversationCaches();
    const payload = '<span style="color:#00ff00">ok</span>';
    const first = parseAnsiSegmentsCached(payload);
    const second = parseAnsiSegmentsCached(payload);
    expect(first).toBe(second);
    const stats = getAcpConversationCacheStats();
    expect(stats.ansiMisses).toBe(1);
    expect(stats.ansiHits).toBe(1);
  });

  it("parses ansi spans when the renderer adds a class attribute", () => {
    resetAcpConversationCaches();
    const payload =
      '<span class="ansi-green" style="color:#00ff00;font-weight:bold">ok</span>';
    expect(parseAnsiSegmentsCached(payload)).toEqual([
      {
        text: "ok",
        style: {
          color: "#00ff00",
          fontWeight: "bold",
        },
      },
    ]);
  });
});
