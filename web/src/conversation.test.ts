import { describe, expect, it } from "vitest";
import { buildConversationMessages } from "./conversation";
import { AcpMessage } from "./acp";

function msg(
  kind: AcpMessage["kind"],
  text: string,
  session_id: string | null
): AcpMessage {
  return {
    kind,
    text,
    session_id,
    message_id: null,
    chunk: false,
  };
}

describe("buildConversationMessages", () => {
  it("attaches pending thought to the next agent message", () => {
    const messages: AcpMessage[] = [
      msg("agent_thought", "t1", "s1"),
      msg("agent_message", "m1", "s1"),
    ];
    const items = buildConversationMessages(messages, "s1");
    expect(items).toHaveLength(2);
    expect(items[0].kind).toBe("agent_thinking");
    expect(items[0].text).toBe("t1");
    expect(items[0].live).toBe(false);
    expect(items[1].kind).toBe("agent_message");
    expect(items[1].text).toBe("m1");
  });

  it("emits trailing thought as its own item", () => {
    const messages: AcpMessage[] = [msg("agent_thought", "t1", "s1")];
    const items = buildConversationMessages(messages, "s1");
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
    const items = buildConversationMessages(messages, "s1");
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
    const items = buildConversationMessages(messages, "s1");
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
    const items = buildConversationMessages(messages, "s1");
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
    const items = buildConversationMessages(messages, "s1");
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("agent_message");
    expect(items[0].text).toBe("m1");
  });
});
