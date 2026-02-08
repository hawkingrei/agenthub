import { describe, expect, it } from "vitest";
import {
  applyConversationFreeze,
  buildConversationMessages,
  deriveConversationFreezeMaxSeq,
  formatConversationPreview,
  isToolCallLive,
} from "./conversation";
import { AcpMessage, AcpToolCall } from "./acp";

function msg(
  kind: AcpMessage["kind"],
  text: string,
  session_id: string | null,
  seq?: number
): AcpMessage {
  return {
    kind,
    text,
    session_id,
    message_id: null,
    seq,
    chunk: false,
  };
}

function toolCall(
  id: string,
  title: string,
  session_id: string | null,
  seq: number
): AcpToolCall {
  return {
    id,
    title,
    session_id,
    seq,
  };
}

describe("buildConversationMessages", () => {
  it("attaches pending thought to the next agent message", () => {
    const messages: AcpMessage[] = [
      msg("agent_thought", "t1", "s1"),
      msg("agent_message", "m1", "s1"),
    ];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(2);
    expect(items[0].kind).toBe("agent_thinking");
    expect(items[0].text).toBe("t1");
    expect(items[0].live).toBe(false);
    expect(items[1].kind).toBe("agent_message");
    expect(items[1].text).toBe("m1");
  });

  it("emits trailing thought as its own item", () => {
    const messages: AcpMessage[] = [msg("agent_thought", "t1", "s1")];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("agent_thinking");
    expect(items[0].text).toBe("t1");
    expect(items[0].live).toBe(true);
  });

  it("concats consecutive thoughts with newline", () => {
    const messages: AcpMessage[] = [
      msg("agent_thought", "t1", "s1"),
      msg("agent_thought", "t2", "s1"),
      msg("agent_message", "m1", "s1"),
    ];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(2);
    expect(items[0].kind).toBe("agent_thinking");
    expect(items[0].text).toBe("t1\nt2");
    expect(items[0].live).toBe(false);
    expect(items[1].kind).toBe("agent_message");
  });

  it("filters by session id while keeping session-less messages", () => {
    const messages: AcpMessage[] = [
      msg("user_message", "u1", "s1"),
      msg("agent_message", "m1", "s2"),
      msg("agent_message", "m2", null),
    ];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(2);
    expect(items[0].text).toBe("u1");
    expect(items[1].text).toBe("m2");
  });

  it("emits thinking before user message when it appears in sequence", () => {
    const messages: AcpMessage[] = [
      msg("agent_thought", "t1", "s1"),
      msg("user_message", "u1", "s1"),
      msg("agent_message", "m1", "s1"),
    ];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(3);
    expect(items[0].kind).toBe("agent_thinking");
    expect(items[0].text).toBe("t1");
    expect(items[1].kind).toBe("user_message");
    expect(items[1].text).toBe("u1");
    expect(items[2].kind).toBe("agent_message");
  });

  it("does not leak thoughts from other sessions", () => {
    const messages: AcpMessage[] = [
      msg("agent_thought", "t1", "s2"),
      msg("agent_message", "m1", "s1"),
    ];
    const items = buildConversationMessages(messages, [], null, "s1");
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("agent_message");
    expect(items[0].text).toBe("m1");
  });

  it("merges tool calls into the conversation flow", () => {
    const messages: AcpMessage[] = [
      msg("user_message", "u1", "s1", 1),
      msg("agent_thought", "t1", "s1", 2),
      msg("agent_message", "m1", "s1", 4),
    ];
    const calls = [toolCall("c1", "Tool A", "s1", 3)];
    const items = buildConversationMessages(messages, calls, null, "s1");
    expect(items).toHaveLength(4);
    expect(items[0].kind).toBe("user_message");
    expect(items[1].kind).toBe("agent_thinking");
    expect(items[2].kind).toBe("tool_call");
    expect(items[3].kind).toBe("agent_message");
  });

  it("filters tool calls by session id", () => {
    const messages: AcpMessage[] = [msg("agent_message", "m1", "s1", 1)];
    const calls = [
      toolCall("c1", "Tool A", "s2", 2),
      toolCall("c2", "Tool B", null, 3),
    ];
    const items = buildConversationMessages(messages, calls, null, "s1");
    expect(items).toHaveLength(2);
    expect(items[1].kind).toBe("tool_call");
  });

  it("renders plan entries as a conversation item", () => {
    const messages: AcpMessage[] = [msg("agent_message", "m1", "s1", 1)];
    const plan = {
      entries: [{ content: "Do X", status: "todo" }],
      session_id: "s1",
      seq: 2,
    };
    const items = buildConversationMessages(messages, [], plan, "s1");
    expect(items).toHaveLength(2);
    expect(items[1].kind).toBe("agent_plan");
    expect(items[1].text).toContain("Do X");
  });

  it("places plan based on sequence ordering", () => {
    const messages: AcpMessage[] = [
      msg("user_message", "u1", "s1", 1),
      msg("agent_message", "m1", "s1", 3),
    ];
    const plan = {
      entries: [{ content: "Plan first" }],
      session_id: "s1",
      seq: 2,
    };
    const items = buildConversationMessages(messages, [], plan, "s1");
    expect(items.map((item) => item.kind)).toEqual([
      "user_message",
      "agent_plan",
      "agent_message",
    ]);
  });

  it("filters plan entries by session id", () => {
    const messages: AcpMessage[] = [msg("agent_message", "m1", "s1", 1)];
    const plan = {
      entries: [{ content: "Do X" }],
      session_id: "s2",
      seq: 2,
    };
    const items = buildConversationMessages(messages, [], plan, "s1");
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("agent_message");
  });
});

describe("conversation freeze helpers", () => {
  it("derives max seq from items", () => {
    const items = [
      { kind: "user_message", text: "a", seq: 1 },
      { kind: "agent_message", text: "b", seq: 3 },
      { kind: "agent_thinking", text: "c", seq: 2 },
    ];
    expect(deriveConversationFreezeMaxSeq(items)).toBe(3);
  });

  it("filters items beyond max seq and counts pending", () => {
    const items = [
      { kind: "user_message", text: "a", seq: 1 },
      { kind: "agent_message", text: "b", seq: 2 },
      { kind: "agent_message", text: "c", seq: 4 },
    ];
    const result = applyConversationFreeze(items, 2);
    expect(result.frozen).toHaveLength(2);
    expect(result.pending).toBe(1);
  });
});

describe("tool call status helpers", () => {
  it("treats pending and in_progress as live", () => {
    expect(isToolCallLive("pending")).toBe(true);
    expect(isToolCallLive("in_progress")).toBe(true);
  });

  it("treats other statuses as not live", () => {
    expect(isToolCallLive("completed")).toBe(false);
    expect(isToolCallLive("failed")).toBe(false);
    expect(isToolCallLive(undefined)).toBe(false);
  });
});

describe("formatConversationPreview", () => {
  it("returns empty string for empty input", () => {
    expect(formatConversationPreview("   ", 80)).toBe("");
  });

  it("normalizes whitespace", () => {
    expect(formatConversationPreview("a\nb\tc", 80)).toBe("a b c");
  });

  it("truncates long text", () => {
    const text = "a".repeat(90);
    expect(formatConversationPreview(text, 80)).toBe(`${"a".repeat(80)}…`);
  });

  it("returns empty string when limit is non-positive", () => {
    expect(formatConversationPreview("hello", 0)).toBe("");
    expect(formatConversationPreview("hello", -5)).toBe("");
  });
});
