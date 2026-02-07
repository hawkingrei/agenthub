import { AcpMessage, AcpPlanView, AcpToolCall } from "./acp";

export type ConversationItem =
  | {
      kind: "user_message" | "agent_message" | "agent_thinking" | "agent_plan";
      text: string;
      live?: boolean;
      seq?: number;
    }
  | {
      kind: "tool_call";
      title: string;
      status?: string;
      content?: string;
      raw_input?: unknown;
      raw_output?: unknown;
      terminal_output?: string;
      seq?: number;
    };

export type ConversationWindow = {
  items: ConversationItem[];
  offset: number;
  total: number;
};

export function isToolCallLive(status?: string): boolean {
  if (!status) return false;
  const normalized = status.toLowerCase();
  return normalized === "pending" || normalized === "in_progress";
}

export function formatConversationPreview(text: string, limit: number): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, limit)}…`;
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
  const planSeq =
    plan?.seq ??
    [...filteredMessages, ...filteredToolCalls]
      .map((item) => item.seq)
      .reduce<number | null>(
        (acc, seq) =>
          typeof seq === "number" ? Math.max(acc ?? seq, seq) : acc,
        null
      );
  const entries: Array<{
    kind: "message" | "tool_call";
    seq: number | null;
    order: number;
    message?: AcpMessage;
    toolCall?: AcpToolCall;
  }> = [];
  let order = 0;
  for (const msg of filteredMessages) {
    entries.push({
      kind: "message",
      seq: msg.seq ?? null,
      order,
      message: msg,
    });
    order += 1;
  }
  for (const call of filteredToolCalls) {
    entries.push({
      kind: "tool_call",
      seq: call.seq ?? null,
      order,
      toolCall: call,
    });
    order += 1;
  }
  entries.sort((a, b) => {
    if (a.seq == null && b.seq == null) return a.order - b.order;
    if (a.seq == null) return 1;
    if (b.seq == null) return -1;
    if (a.seq !== b.seq) return a.seq - b.seq;
    return a.order - b.order;
  });
  const items: ConversationItem[] = [];
  let pendingThought: string | null = null;
  let pendingThoughtSeq: number | null = null;
  for (const entry of entries) {
    if (entry.kind === "message") {
      const msg = entry.message;
      if (!msg) continue;
      if (msg.kind === "agent_thought") {
        pendingThought = pendingThought ? `${pendingThought}\n${msg.text}` : msg.text;
        pendingThoughtSeq = msg.seq ?? pendingThoughtSeq;
        continue;
      }
      if (pendingThought) {
        items.push({
          kind: "agent_thinking",
          text: pendingThought,
          live: false,
          seq: pendingThoughtSeq ?? msg.seq,
        });
        pendingThought = null;
        pendingThoughtSeq = null;
      }
      if (msg.kind === "agent_message") {
        items.push({ kind: "agent_message", text: msg.text, seq: msg.seq });
        continue;
      }
      items.push({ kind: "user_message", text: msg.text, seq: msg.seq });
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
      });
      pendingThought = null;
      pendingThoughtSeq = null;
    }
    items.push({
      kind: "tool_call",
      title: call.title,
      status: call.status,
      content: call.content,
      raw_input: call.raw_input,
      raw_output: call.raw_output,
      terminal_output: call.terminal_output,
      seq: call.seq,
    });
  }
  if (pendingThought) {
    items.push({
      kind: "agent_thinking",
      text: pendingThought,
      live: true,
      seq: pendingThoughtSeq ?? undefined,
    });
  }
  if (includePlan) {
    const planText = formatPlanEntries(plan.entries);
    if (planText) {
      items.push({
        kind: "agent_plan",
        text: planText,
        live: false,
        seq: planSeq ?? undefined,
      });
    }
  }
  return items;
}

export function windowConversation(
  items: ConversationItem[],
  stickToBottom: boolean,
  windowSize: number
): ConversationWindow {
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

export function deriveConversationFreezeMaxSeq(
  items: ConversationItem[]
): number | null {
  let maxSeq: number | null = null;
  for (const item of items) {
    if (typeof item.seq !== "number") continue;
    maxSeq = maxSeq === null ? item.seq : Math.max(maxSeq, item.seq);
  }
  return maxSeq;
}

export function applyConversationFreeze(
  items: ConversationItem[],
  maxSeq: number
): { frozen: ConversationItem[]; pending: number } {
  const frozen: ConversationItem[] = [];
  let pending = 0;
  for (const item of items) {
    const seq = item.seq;
    if (typeof seq !== "number" || seq <= maxSeq) {
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
