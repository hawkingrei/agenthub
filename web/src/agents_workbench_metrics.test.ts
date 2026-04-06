import { describe, expect, it } from "vitest";
import { EMPTY_ACP_VIEW } from "./acp";
import { buildAcpRuntimeMetrics } from "./components/agents_workbench_metrics";

describe("buildAcpRuntimeMetrics", () => {
  it("combines conversation counters with ACP/cache stats", () => {
    const acpView = {
      ...EMPTY_ACP_VIEW,
      rawEvents: [{ ts: 1, type: "message", payload: {} }],
      toolCalls: [{ id: "tool-1", title: "Tool 1" }],
      messages: [
        {
          kind: "agent_message" as const,
          text: "hello",
          chunk: false,
        },
      ],
    };

    expect(
      buildAcpRuntimeMetrics({
        rawEventCount: acpView.rawEvents.length,
        toolCallCount: acpView.toolCalls.length,
        messageCount: acpView.messages.length,
        conversation: {
          totalItems: 12,
          sourceItems: 16,
          renderedItems: 10,
          pendingItems: 2,
          virtualized: true,
          stickToBottom: false,
          averageHeight: 48.6,
        },
        cacheStats: {
          markdownHits: 7,
          markdownMisses: 3,
          ansiHits: 5,
          ansiMisses: 1,
          payloadParses: 9,
          payloadParseFailures: 2,
        },
      })
    ).toEqual({
      totalConversationItems: 12,
      sourceConversationItems: 16,
      renderedConversationItems: 10,
      pendingConversationItems: 2,
      virtualizedConversation: true,
      stickToBottom: false,
      averageConversationHeight: 49,
      rawEventCount: 1,
      toolCallCount: 1,
      messageCount: 1,
      markdownCacheHits: 7,
      markdownCacheMisses: 3,
      ansiCacheHits: 5,
      ansiCacheMisses: 1,
      payloadParses: 9,
      payloadParseFailures: 2,
    });
  });
});
