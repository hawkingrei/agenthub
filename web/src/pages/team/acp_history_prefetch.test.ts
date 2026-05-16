import { describe, expect, it } from "vitest";
import type { AgentEvent } from "../../api";
import {
  countRenderableAcpConversationItems,
  resolveAdaptiveAcpHistoryPageCap,
  resolveInitialAcpHistoryDecision,
  shouldContinueInitialAcpHistoryPrefetch,
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
        name: "renderable but underfilled for a complete leading reply",
        events: [
          acpEvent(1, {
            type: "agent_message",
            text: "Complete reply.",
          }),
        ],
        hasMore: true,
        expectedState: "renderable",
        expectedPrefetch: true,
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
        expectedPrefetch: true,
        expectedRenderableCount: 1,
      },
      {
        name: "renderable enough context stops prefetching",
        events: Array.from({ length: 8 }, (_, index) =>
          acpEvent(index + 1, {
            type: "agent_message",
            text: `Complete reply ${index + 1}.`,
          })
        ),
        hasMore: true,
        expectedState: "renderable",
        expectedPrefetch: false,
        expectedRenderableCount: 8,
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

  it("only continues context prefetch on full pages unless recovering partial-only chunks", () => {
    const underfilledRenderable = resolveInitialAcpHistoryDecision(
      [
        acpEvent(1, {
          type: "agent_message",
          text: "One visible reply.",
        }),
      ],
      "runtime-session-1",
      true
    );
    expect(
      shouldContinueInitialAcpHistoryPrefetch(underfilledRenderable, 1, 60)
    ).toBe(false);
    expect(
      shouldContinueInitialAcpHistoryPrefetch(underfilledRenderable, 60, 60)
    ).toBe(true);

    const partialOnly = resolveInitialAcpHistoryDecision(
      [
        acpEvent(2, {
          type: "agent_message",
          text: "tail chunk",
          chunk: true,
          message_id: "msg-1",
          chunk_index: 32,
        }),
      ],
      "runtime-session-1",
      true
    );
    expect(shouldContinueInitialAcpHistoryPrefetch(partialOnly, 1, 60)).toBe(true);
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
