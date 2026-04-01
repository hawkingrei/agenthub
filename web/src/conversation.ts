import {
  AcpMessage,
  AcpPlanEntry,
  AcpPlanView,
  AcpTerminalActivity,
  AcpToolCall,
} from "./acp";
import { compareEventOrder, type SeqComparable } from "./seq_order";

export type PlanEntryView = Pick<AcpPlanEntry, "content" | "status" | "priority">;

export type MessageConversationItem = {
  kind: "user_message" | "agent_message" | "agent_thinking" | "agent_plan";
  text: string;
  plan_entries?: PlanEntryView[];
  live?: boolean;
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type ToolCallConversationItem = {
  kind: "tool_call";
  id: string;
  title: string;
  status?: string;
  content?: string;
  raw_input?: unknown;
  raw_output?: unknown;
  terminal_output?: string;
  terminal_activities?: AcpTerminalActivity[];
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type ToolCallGroupConversationItem = {
  kind: "tool_call_group";
  calls: ToolCallConversationItem[];
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type ExploreGroupChildConversationItem =
  | (MessageConversationItem & { kind: "agent_thinking" })
  | ToolCallConversationItem
  | ToolCallGroupConversationItem;

export type ExploreGroupConversationItem = {
  kind: "explore_group";
  items: ExploreGroupChildConversationItem[];
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type ConversationItem =
  | MessageConversationItem
  | ToolCallConversationItem
  | ToolCallGroupConversationItem
  | ExploreGroupConversationItem;

export type ConversationWindow<T = ConversationItem> = {
  items: T[];
  offset: number;
  total: number;
};

export function isToolCallLive(status?: string): boolean {
  if (!status) return false;
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  return (
    normalized === "pending" ||
    normalized === "in_progress" ||
    normalized === "running"
  );
}

export function formatConversationPreview(text: string, limit: number): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (limit <= 0) return "";
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, limit)}…`;
}

export function isExploreThinkingText(text: string): boolean {
  const firstLine = text
    .split("\n")
    .map((line) => line.trim().toLowerCase())
    .find((line) => line.length > 0);
  return Boolean(firstLine?.startsWith("explore"));
}

export function flattenExploreGroupToolCalls(
  items: ExploreGroupConversationItem["items"]
): ToolCallConversationItem[] {
  const calls: ToolCallConversationItem[] = [];
  for (const item of items) {
    if (item.kind === "tool_call") {
      calls.push(item);
      continue;
    }
    if (item.kind === "tool_call_group") {
      calls.push(...item.calls);
    }
  }
  return calls;
}

export function buildConversationMessages(
  messages: AcpMessage[],
  toolCalls: AcpToolCall[],
  plan: AcpPlanView | null,
  activeSessionId: string | null
): ConversationItem[] {
  const filteredMessages = activeSessionId
    ? messages.filter(
        (msg) => msg.session_id === activeSessionId || msg.session_id == null
      )
    : messages;
  const filteredToolCalls = activeSessionId
    ? toolCalls.filter(
        (call) => call.session_id === activeSessionId || call.session_id == null
      )
    : toolCalls;
  const includePlan =
    plan &&
    (!activeSessionId ||
      plan.session_id == null ||
      plan.session_id === activeSessionId);
  const planText = includePlan && plan ? formatPlanEntries(plan.entries) : "";
  const entries: Array<{
    kind: "message" | "tool_call" | "plan";
    event_id: number | null;
    seq: string | null;
    ts: number | null;
    order: number;
    message?: AcpMessage;
    toolCall?: AcpToolCall;
    plan?: AcpPlanView;
  }> = [];
  let order = 0;
  for (const msg of filteredMessages) {
    entries.push({
      kind: "message",
      event_id: msg.event_id ?? null,
      seq: msg.seq ?? null,
      ts: msg.ts ?? null,
      order,
      message: msg,
    });
    order += 1;
  }
  for (const call of filteredToolCalls) {
    entries.push({
      kind: "tool_call",
      event_id: call.event_id ?? null,
      seq: call.seq ?? null,
      ts: call.ts ?? null,
      order,
      toolCall: call,
    });
    order += 1;
  }
  if (includePlan && plan && planText) {
    entries.push({
      kind: "plan",
      event_id: plan.event_id ?? null,
      seq: plan.seq ?? null,
      ts: plan.ts ?? null,
      order,
      plan,
    });
  }
  entries.sort((a, b) => {
    const base = compareEventOrder(
      { event_id: a.event_id ?? null, ts: a.ts ?? null },
      { event_id: b.event_id ?? null, ts: b.ts ?? null }
    );
    if (base !== 0) return base;
    return a.order - b.order;
  });
  const items: ConversationItem[] = [];
  let pendingThought: string | null = null;
  let pendingThoughtSeq: string | null = null;
  let pendingThoughtEventId: number | null = null;
  let pendingThoughtTs: number | null = null;
  for (const entry of entries) {
    if (entry.kind === "message") {
      const msg = entry.message;
      if (!msg) continue;
      if (msg.kind === "agent_thought") {
        pendingThought = pendingThought ? `${pendingThought}\n${msg.text}` : msg.text;
        pendingThoughtSeq = msg.seq ?? pendingThoughtSeq;
        pendingThoughtEventId = msg.event_id ?? pendingThoughtEventId;
        pendingThoughtTs = msg.ts ?? pendingThoughtTs;
        continue;
      }
      if (pendingThought) {
        items.push({
          kind: "agent_thinking",
          text: pendingThought,
          live: false,
          seq: pendingThoughtSeq ?? msg.seq,
          event_id: pendingThoughtEventId ?? entry.event_id ?? undefined,
          ts: pendingThoughtTs ?? entry.ts ?? undefined,
        });
        pendingThought = null;
        pendingThoughtSeq = null;
        pendingThoughtEventId = null;
        pendingThoughtTs = null;
      }
      if (msg.kind === "agent_message") {
        items.push({
          kind: "agent_message",
          text: msg.text,
          seq: msg.seq,
          event_id: msg.event_id,
          ts: msg.ts,
        });
        continue;
      }
      items.push({
        kind: "user_message",
        text: msg.text,
        seq: msg.seq,
        event_id: msg.event_id,
        ts: msg.ts,
      });
      continue;
    }
    if (entry.kind === "plan") {
      if (pendingThought) {
        items.push({
          kind: "agent_thinking",
          text: pendingThought,
          live: false,
          seq: pendingThoughtSeq ?? entry.seq ?? undefined,
          event_id: pendingThoughtEventId ?? entry.event_id ?? undefined,
          ts: pendingThoughtTs ?? entry.ts ?? undefined,
        });
        pendingThought = null;
        pendingThoughtSeq = null;
        pendingThoughtEventId = null;
        pendingThoughtTs = null;
      }
      items.push({
        kind: "agent_plan",
        text: planText,
        plan_entries: plan.entries?.map((entry) => ({
          content: entry.content,
          status: entry.status,
          priority: entry.priority,
        })),
        live: false,
        seq: entry.seq ?? undefined,
        event_id: entry.event_id ?? undefined,
        ts: entry.ts ?? undefined,
      });
      continue;
    }
    const call = entry.toolCall;
    if (!call) continue;
    if (pendingThought) {
      items.push({
        kind: "agent_thinking",
        text: pendingThought,
        live: false,
        seq: pendingThoughtSeq ?? call.seq,
        event_id: pendingThoughtEventId ?? call.event_id,
        ts: pendingThoughtTs ?? call.ts,
      });
      pendingThought = null;
      pendingThoughtSeq = null;
      pendingThoughtEventId = null;
      pendingThoughtTs = null;
    }
    items.push({
      kind: "tool_call",
      id: call.id,
      title: call.title,
      status: call.status,
      content: call.content,
      raw_input: call.raw_input,
      raw_output: call.raw_output,
      terminal_output: call.terminal_output,
      terminal_activities: call.terminal_activities,
      seq: call.seq,
      event_id: call.event_id,
      ts: call.ts,
    });
  }
  if (pendingThought) {
    items.push({
      kind: "agent_thinking",
      text: pendingThought,
      live: true,
      seq: pendingThoughtSeq ?? undefined,
      event_id: pendingThoughtEventId ?? undefined,
      ts: pendingThoughtTs ?? undefined,
    });
  }
  return groupExploreItems(groupConsecutiveToolCalls(items));
}

function groupConsecutiveToolCalls(items: ConversationItem[]): ConversationItem[] {
  if (items.length < 2) return items;
  const grouped: ConversationItem[] = [];
  let pendingCalls: ToolCallConversationItem[] = [];

  const flushPendingCalls = () => {
    if (pendingCalls.length === 0) return;
    if (pendingCalls.length === 1) {
      grouped.push(pendingCalls[0]);
    } else {
      const tail = pendingCalls[pendingCalls.length - 1];
      grouped.push({
        kind: "tool_call_group",
        calls: pendingCalls,
        seq: tail.seq,
        event_id: tail.event_id,
        ts: tail.ts,
      });
    }
    pendingCalls = [];
  };

  for (const item of items) {
    if (item.kind === "tool_call") {
      pendingCalls.push(item);
      continue;
    }
    flushPendingCalls();
    grouped.push(item);
  }
  flushPendingCalls();
  return grouped;
}

function groupExploreItems(items: ConversationItem[]): ConversationItem[] {
  if (items.length < 2) return items;
  const grouped: ConversationItem[] = [];
  let index = 0;

  while (index < items.length) {
    const current = items[index];
    if (!isExploreThinkingItem(current)) {
      grouped.push(current);
      index += 1;
      continue;
    }

    const runItems: ExploreGroupChildConversationItem[] = [current];
    let hasToolCall = false;
    index += 1;

    while (index < items.length) {
      const next = items[index];
      if (next.kind === "tool_call" || next.kind === "tool_call_group") {
        runItems.push(next);
        hasToolCall = true;
        index += 1;
        continue;
      }
      if (isExploreThinkingItem(next)) {
        runItems.push(next);
        index += 1;
        continue;
      }
      break;
    }

    if (!hasToolCall) {
      grouped.push(current);
      continue;
    }

    const tail = runItems[runItems.length - 1];
    grouped.push({
      kind: "explore_group",
      items: runItems,
      seq: tail.seq,
      event_id: tail.event_id,
      ts: tail.ts,
    });
  }

  return grouped;
}

function isExploreThinkingItem(
  item: ConversationItem
): item is MessageConversationItem & { kind: "agent_thinking" } {
  return item.kind === "agent_thinking" && isExploreThinkingText(item.text);
}

export function windowConversation<T>(
  items: T[],
  stickToBottom: boolean,
  windowSize: number
): ConversationWindow<T> {
  const total = items.length;
  if (windowSize <= 0 || total <= windowSize || !stickToBottom) {
    return { items, offset: 0, total };
  }
  const offset = Math.max(0, total - windowSize);
  return {
    items: items.slice(offset),
    offset,
    total,
  };
}

export function deriveConversationFreezeCursor(
  items: ConversationItem[]
): SeqComparable | null {
  let maxCursor: SeqComparable | null = null;
  for (const item of items) {
    const cursor: SeqComparable = {
      event_id: item.event_id ?? null,
      ts: item.ts ?? null,
    };
    if (maxCursor == null) {
      maxCursor = cursor;
      continue;
    }
    if (compareEventOrder(cursor, maxCursor) > 0) {
      maxCursor = cursor;
    }
  }
  return maxCursor;
}

export function applyConversationFreeze(
  items: ConversationItem[],
  maxCursor: SeqComparable | null
): { frozen: ConversationItem[]; pending: number } {
  const frozen: ConversationItem[] = [];
  let pending = 0;
  for (const item of items) {
    const cursor: SeqComparable = {
      event_id: item.event_id ?? null,
      ts: item.ts ?? null,
    };
    if (maxCursor == null || compareEventOrder(cursor, maxCursor) <= 0) {
      frozen.push(item);
      continue;
    }
    pending += 1;
  }
  return { frozen, pending };
}

function formatPlanEntries(entries: AcpPlanView["entries"]): string {
  if (!entries || entries.length === 0) return "";
  return entries
    .map((entry, idx) => {
      const title = entry.content ?? "";
      const status = entry.status ? ` [${entry.status}]` : "";
      const prefix = `${idx + 1}. `;
      return `${prefix}${title}${status}`.trimEnd();
    })
    .join("\n");
}
