import { AcpMessage } from "./acp";

export type ConversationItem = {
  kind: "user_message" | "agent_message" | "agent_thinking";
  text: string;
  live?: boolean;
};

export type ConversationWindow = {
  items: ConversationItem[];
  offset: number;
  total: number;
};

export function buildConversationMessages(
  messages: AcpMessage[],
  activeSessionId: string | null
): ConversationItem[] {
  const filtered = activeSessionId
    ? messages.filter(
        (msg) => msg.session_id === activeSessionId || msg.session_id == null
      )
    : messages;
  const items: ConversationItem[] = [];
  let pendingThought: string | null = null;
  for (const msg of filtered) {
    if (msg.kind === "agent_thought") {
      pendingThought = pendingThought ? `${pendingThought}\n${msg.text}` : msg.text;
      continue;
    }
    if (pendingThought) {
      items.push({ kind: "agent_thinking", text: pendingThought, live: false });
      pendingThought = null;
    }
    if (msg.kind === "agent_message") {
      items.push({ kind: "agent_message", text: msg.text });
      continue;
    }
    items.push({ kind: "user_message", text: msg.text });
  }
  if (pendingThought) {
    items.push({ kind: "agent_thinking", text: pendingThought, live: true });
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
