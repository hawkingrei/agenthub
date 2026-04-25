import { buildAcpView } from "../../acp";
import type { AgentEvent } from "../../api";
import { buildConversationMessages } from "../../conversation";

export const ACP_INITIAL_VISIBLE_MESSAGE_TARGET = 1;
export const ACP_HISTORY_PAGE_LIMIT_MAX = 180;
export type InitialAcpHistoryState =
  | "empty"
  | "renderable"
  | "partial_only"
  | "partial_with_renderable_tail";

export type InitialAcpHistoryDecision = {
  state: InitialAcpHistoryState;
  visibleCount: number;
  renderableCount: number;
  hasIncompleteLeadingMessage: boolean;
  shouldPrefetchInitialHistory: boolean;
};

type LeadingAcpMessageChunkState = {
  messageId: string | null;
  chunkIndex: number | null;
  incomplete: boolean;
};

function resolveLeadingAcpMessageChunkState(
  events: AgentEvent[],
  sessionId: string | null | undefined
): LeadingAcpMessageChunkState {
  const scopedSessionId = sessionId ?? null;
  const ordered = [...events].sort((left, right) => left.event_id - right.event_id);
  for (const event of ordered) {
    if (event.stream !== "acp") {
      continue;
    }
    if ((event.session_id ?? null) !== scopedSessionId) {
      continue;
    }
    const trimmed = event.message.trim();
    if (!trimmed.startsWith("{")) {
      continue;
    }
    try {
      const payload = JSON.parse(trimmed) as Record<string, unknown>;
      if (payload.type !== "agent_message") {
        continue;
      }
      const messageId =
        typeof payload.message_id === "string" && payload.message_id.trim().length > 0
          ? payload.message_id
          : null;
      if (payload.chunk !== true) {
        return {
          messageId,
          chunkIndex: 0,
          incomplete: false,
        };
      }
      const chunkIndex =
        typeof payload.chunk_index === "number"
          ? payload.chunk_index
          : typeof payload.chunk_index === "string"
            ? Number.parseInt(payload.chunk_index, 10)
            : Number.NaN;
      return {
        messageId,
        chunkIndex: Number.isFinite(chunkIndex) ? chunkIndex : null,
        incomplete: Number.isFinite(chunkIndex) ? chunkIndex > 0 : true,
      };
    } catch {
      continue;
    }
  }
  return {
    messageId: null,
    chunkIndex: null,
    incomplete: false,
  };
}

export function omitIncompleteLeadingAcpMessageEvents(
  events: AgentEvent[],
  sessionId: string | null | undefined
): AgentEvent[] {
  const leadingState = resolveLeadingAcpMessageChunkState(events, sessionId);
  if (!leadingState.incomplete || !leadingState.messageId) {
    return events;
  }
  const scopedSessionId = sessionId ?? null;
  return events.filter((event) => {
    if (event.stream !== "acp" || (event.session_id ?? null) !== scopedSessionId) {
      return true;
    }
    const trimmed = event.message.trim();
    if (!trimmed.startsWith("{")) {
      return true;
    }
    try {
      const payload = JSON.parse(trimmed) as Record<string, unknown>;
      return !(
        payload.type === "agent_message" &&
        typeof payload.message_id === "string" &&
        payload.message_id === leadingState.messageId
      );
    } catch {
      return true;
    }
  });
}

export function hasIncompleteLeadingAcpMessage(
  events: AgentEvent[],
  sessionId: string | null | undefined
): boolean {
  return resolveLeadingAcpMessageChunkState(events, sessionId).incomplete;
}

export function hasOnlyIncompleteLeadingAcpMessage(
  events: AgentEvent[],
  sessionId: string | null | undefined
): boolean {
  if (!hasIncompleteLeadingAcpMessage(events, sessionId)) {
    return false;
  }
  const visibleEvents = omitIncompleteLeadingAcpMessageEvents(events, sessionId);
  if (countVisibleAcpConversationItems(visibleEvents, sessionId) >= 1) {
    return false;
  }
  return countVisibleAcpConversationItems(events, sessionId) >= 1;
}

export function countVisibleAcpConversationItems(
  events: AgentEvent[],
  sessionId: string | null | undefined
): number {
  const acpEventLines = events.map((event) => ({
    ts: event.ts,
    seq: event.seq,
    event_id: event.event_id,
    stream: event.stream,
    message: event.message,
    session_id: event.session_id,
  }));
  const acpView = buildAcpView(acpEventLines);
  return buildConversationMessages(
    acpView.messages,
    acpView.toolCalls,
    acpView.plan,
    sessionId ?? null
  ).length;
}

export function countRenderableAcpConversationItems(
  events: AgentEvent[],
  sessionId: string | null | undefined
): number {
  return countVisibleAcpConversationItems(
    omitIncompleteLeadingAcpMessageEvents(events, sessionId),
    sessionId
  );
}

export function resolveInitialAcpHistoryDecision(
  events: AgentEvent[],
  sessionId: string | null | undefined,
  hasMore: boolean,
  minVisibleItems: number = ACP_INITIAL_VISIBLE_MESSAGE_TARGET
): InitialAcpHistoryDecision {
  const visibleCount = countVisibleAcpConversationItems(events, sessionId);
  const renderableCount = countRenderableAcpConversationItems(events, sessionId);
  const hasIncompleteLeadingMessage = hasIncompleteLeadingAcpMessage(events, sessionId);
  const hasOnlyPartialLeadingMessage =
    hasIncompleteLeadingMessage &&
    renderableCount < minVisibleItems &&
    visibleCount >= minVisibleItems;

  let state: InitialAcpHistoryState;
  if (renderableCount >= minVisibleItems) {
    state = hasIncompleteLeadingMessage
      ? "partial_with_renderable_tail"
      : "renderable";
  } else if (hasOnlyPartialLeadingMessage) {
    state = "partial_only";
  } else {
    state = "empty";
  }

  return {
    state,
    visibleCount,
    renderableCount,
    hasIncompleteLeadingMessage,
    shouldPrefetchInitialHistory:
      hasMore &&
      (state === "partial_only" || visibleCount < minVisibleItems),
  };
}

export function shouldPrefetchInitialAcpHistory(
  events: AgentEvent[],
  sessionId: string | null | undefined,
  hasMore: boolean,
  minVisibleItems: number = ACP_INITIAL_VISIBLE_MESSAGE_TARGET
): boolean {
  return resolveInitialAcpHistoryDecision(
    events,
    sessionId,
    hasMore,
    minVisibleItems
  ).shouldPrefetchInitialHistory;
}

function countLeadingMessageChunkEvents(
  events: AgentEvent[],
  sessionId: string | null | undefined,
  messageId: string | null
): { sameMessageChunkCount: number; scopedAcpEventCount: number } {
  if (!messageId) {
    return { sameMessageChunkCount: 0, scopedAcpEventCount: 0 };
  }
  const scopedSessionId = sessionId ?? null;
  let sameMessageChunkCount = 0;
  let scopedAcpEventCount = 0;
  for (const event of events) {
    if (event.stream !== "acp" || (event.session_id ?? null) !== scopedSessionId) {
      continue;
    }
    scopedAcpEventCount += 1;
    const trimmed = event.message.trim();
    if (!trimmed.startsWith("{")) {
      continue;
    }
    try {
      const payload = JSON.parse(trimmed) as Record<string, unknown>;
      if (
        payload.type === "agent_message" &&
        payload.chunk === true &&
        typeof payload.message_id === "string" &&
        payload.message_id === messageId
      ) {
        sameMessageChunkCount += 1;
      }
    } catch {
      continue;
    }
  }
  return { sameMessageChunkCount, scopedAcpEventCount };
}

export function resolveAdaptiveAcpHistoryPageLimit(
  events: AgentEvent[],
  sessionId: string | null | undefined,
  baseLimit: number
): number {
  const leadingState = resolveLeadingAcpMessageChunkState(events, sessionId);
  if (!leadingState.incomplete || leadingState.chunkIndex == null) {
    return baseLimit;
  }
  const { sameMessageChunkCount, scopedAcpEventCount } = countLeadingMessageChunkEvents(
    events,
    sessionId,
    leadingState.messageId
  );
  const leadingMessageDominatesPage =
    sameMessageChunkCount >= 8 &&
    scopedAcpEventCount > 0 &&
    sameMessageChunkCount / scopedAcpEventCount >= 0.6;
  if (leadingState.chunkIndex >= 192) {
    return Math.max(baseLimit, ACP_HISTORY_PAGE_LIMIT_MAX);
  }
  if (leadingState.chunkIndex >= 96) {
    return Math.max(baseLimit, ACP_HISTORY_PAGE_LIMIT_MAX);
  }
  if (leadingState.chunkIndex >= 32 || leadingMessageDominatesPage) {
    return Math.max(baseLimit, 120);
  }
  return baseLimit;
}
