import { buildAcpView } from "../../acp";
import type { AgentEvent } from "../../api";
import { buildConversationMessages } from "../../conversation";

export const ACP_INITIAL_VISIBLE_MESSAGE_TARGET = 1;
export const ACP_HISTORY_PAGE_LIMIT_MAX = 240;

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

export function hasIncompleteLeadingAcpMessage(
  events: AgentEvent[],
  sessionId: string | null | undefined
): boolean {
  return resolveLeadingAcpMessageChunkState(events, sessionId).incomplete;
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

export function shouldPrefetchInitialAcpHistory(
  events: AgentEvent[],
  sessionId: string | null | undefined,
  hasMore: boolean,
  minVisibleItems: number = ACP_INITIAL_VISIBLE_MESSAGE_TARGET
): boolean {
  if (!hasMore) {
    return false;
  }
  if (hasIncompleteLeadingAcpMessage(events, sessionId)) {
    return true;
  }
  return countVisibleAcpConversationItems(events, sessionId) < minVisibleItems;
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
  if (leadingState.chunkIndex >= 256) {
    return Math.max(baseLimit, ACP_HISTORY_PAGE_LIMIT_MAX);
  }
  if (leadingState.chunkIndex >= 128) {
    return Math.max(baseLimit, 180);
  }
  if (leadingState.chunkIndex >= 64) {
    return Math.max(baseLimit, 120);
  }
  return baseLimit;
}
