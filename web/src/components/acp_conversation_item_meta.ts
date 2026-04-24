import { ConversationItem, flattenExploreGroupToolCalls } from "../conversation";

export function getConversationItemKey(msg: ConversationItem, fallback: number): string {
  if (msg.kind === "tool_call") return `tool_call:${msg.id}`;
  if (msg.kind === "tool_call_group") {
    const ids = msg.calls.map((call) => call.id).join(",");
    return `tool_call_group:${ids}`;
  }
  if (msg.kind === "explore_group") {
    const ids = flattenExploreGroupToolCalls(msg.items)
      .map((call) => call.id)
      .join(",");
    const fallbackKey = msg.event_id ?? msg.seq ?? fallback;
    return `explore_group:${ids || fallbackKey}`;
  }
  if (msg.event_id != null) return `${msg.kind}:event:${msg.event_id}`;
  if (msg.seq) return `${msg.kind}:seq:${msg.seq}`;
  return `${msg.kind}:idx:${fallback}`;
}

export function getConversationItemToolCallId(msg: ConversationItem): string | undefined {
  if (msg.kind === "tool_call") return msg.id;
  return undefined;
}

export function isConversationItemFocusedToolCall(
  msg: ConversationItem,
  focusedToolCallId: string | null
): boolean {
  if (!focusedToolCallId) return false;
  if (msg.kind === "tool_call") return msg.id === focusedToolCallId;
  if (msg.kind === "tool_call_group") {
    return msg.calls.some((call) => call.id === focusedToolCallId);
  }
  if (msg.kind === "explore_group") {
    return flattenExploreGroupToolCalls(msg.items).some(
      (call) => call.id === focusedToolCallId
    );
  }
  return false;
}
