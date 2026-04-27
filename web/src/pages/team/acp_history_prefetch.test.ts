import { describe, expect, it } from "vitest";
import type { AgentEvent } from "../../api";
import {
  countRenderableAcpConversationItems,
  resolveAdaptiveAcpHistoryPageCap,
  resolveInitialAcpHistoryDecision,
} from "./acp_history_prefetch";

function acpEvent(
  eventId: number,
  payload: Record<string, unknown>,
  sessionId = "runtime-session-1"
): AgentEvent {
  return {
    event_id: eventId,
    agent_id: "worker-agent",
    session_id: sessionId,
    seq: String(eventId),
    ts: eventId,
    stream: "acp",
    message: JSON.stringify(payload),
  };
}

describe("resolveInitialAcpHistoryDecision", () => {
  it("classifies initial ACP history states with table-driven cases", () => {
    const cases: Array<{
      name: string;
      events: AgentEvent[];
      hasMore: boolean;
      expectedState: "empty" | "renderable" | "partial_only" | "partial_with_renderable_tail";
      expectedPrefetch: boolean;
      expectedRenderableCount: number;
    }> = [
      {
        name: "empty when there are no ACP messages",
        events: [],
        hasMore: true,
        expectedState: "empty",
        expectedPrefetch: true,
        expectedRenderableCount: 0,
      },
      {
        name: "renderable for a complete leading reply",
        events: [
          acpEvent(1, {
            type: "agent_message",
            text: "Complete reply.",
          }),
        ],
        hasMore: true,
        expectedState: "renderable",
        expectedPrefetch: false,
        expectedRenderableCount: 1,
      },
      {
        name: "partial_only for a trailing chunk without a complete tail",
        events: [
          acpEvent(10, {
            type: "agent_message",
            text: "chunk-128",
            chunk: true,
            message_id: "msg-1",
            chunk_index: 128,
          }),
          acpEvent(11, {
            type: "session_update",
            status: "running",
          }),
        ],
        hasMore: true,
        expectedState: "partial_only",
        expectedPrefetch: true,
        expectedRenderableCount: 0,
      },
      {
        name: "partial_with_renderable_tail when a complete visible reply follows",
        events: [
          acpEvent(10, {
            type: "agent_message",
            text: "chunk-128",
            chunk: true,
            message_id: "msg-1",
            chunk_index: 128,
          }),
          acpEvent(12, {
            type: "agent_message",
            text: "Visible complete reply.",
          }),
        ],
        hasMore: true,
        expectedState: "partial_with_renderable_tail",
        expectedPrefetch: false,
        expectedRenderableCount: 1,
      },
      {
        name: "stops prefetching when there are no older events left",
        events: [
          acpEvent(10, {
            type: "agent_message",
            text: "chunk-128",
            chunk: true,
            message_id: "msg-1",
            chunk_index: 128,
          }),
        ],
        hasMore: false,
        expectedState: "partial_only",
        expectedPrefetch: false,
        expectedRenderableCount: 0,
      },
    ];

    for (const testCase of cases) {
      const decision = resolveInitialAcpHistoryDecision(
        testCase.events,
        "runtime-session-1",
        testCase.hasMore
      );
      expect(decision.state, testCase.name).toBe(testCase.expectedState);
      expect(decision.shouldPrefetchInitialHistory, testCase.name).toBe(
        testCase.expectedPrefetch
      );
      expect(decision.renderableCount, testCase.name).toBe(
        testCase.expectedRenderableCount
      );
    }
  });

  it("treats partial-only warm caches as non-renderable", () => {
    const events = [
      acpEvent(30, {
        type: "agent_message",
        text: "tail chunk",
        chunk: true,
        message_id: "msg-9",
        chunk_index: 140,
      }),
    ];

    expect(countRenderableAcpConversationItems(events, "runtime-session-1")).toBe(0);
  });

  it("widens bounded recovery page cap for very high chunk indexes", () => {
    const events = Array.from({ length: 20 }, (_, index) =>
      acpEvent(100 + index, {
        type: "agent_message",
        text: `chunk-${index}`,
        chunk: true,
        message_id: "msg-9",
        chunk_index: 471 + index,
      })
    );

    expect(resolveAdaptiveAcpHistoryPageCap(events, "runtime-session-1", 6)).toBe(25);
  });
});
