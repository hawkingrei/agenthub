import { describe, expect, it } from "vitest";
import {
  deriveToolCallOpenState,
  getAcpConversationCacheStats,
  isToolCallEffectivelyLive,
  parseAnsiSegmentsCached,
  renderMarkdownCached,
  resetAcpConversationCaches,
  shouldCollapseToolFoldWhenOutOfView,
} from "./components/acp_conversation";

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
});
