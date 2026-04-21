import { buildAcpView } from "../../acp";
import type { AgentEvent } from "../../api";
import { buildConversationMessages } from "../../conversation";

export const ACP_INITIAL_VISIBLE_MESSAGE_TARGET = 2;

export function hasIncompleteLeadingAcpMessage(
  events: AgentEvent[],
  sessionId: string | null | undefined
): boolean {
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
      if (payload.chunk !== true) {
        return false;
      }
      const chunkIndex =
        typeof payload.chunk_index === "number"
          ? payload.chunk_index
          : typeof payload.chunk_index === "string"
            ? Number.parseInt(payload.chunk_index, 10)
            : Number.NaN;
      return Number.isFinite(chunkIndex) && chunkIndex > 0;
    } catch {
      continue;
    }
  }
  return false;
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
