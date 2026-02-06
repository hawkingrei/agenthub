import { describe, expect, it } from "vitest";
import { windowConversation, ConversationItem } from "./conversation";

function item(kind: ConversationItem["kind"], text: string): ConversationItem {
  return { kind, text };
}

describe("windowConversation", () => {
  it("returns full list when stickToBottom is false", () => {
    const items = [item("user_message", "a"), item("agent_message", "b")];
    const windowed = windowConversation(items, false, 1);
    expect(windowed.items).toHaveLength(2);
    expect(windowed.offset).toBe(0);
    expect(windowed.total).toBe(2);
  });

  it("returns tail window when stickToBottom is true", () => {
    const items = [
      item("user_message", "a"),
      item("agent_message", "b"),
      item("agent_thinking", "c"),
      item("agent_message", "d"),
    ];
    const windowed = windowConversation(items, true, 2);
    expect(windowed.items).toHaveLength(2);
    expect(windowed.items[0].text).toBe("c");
    expect(windowed.items[1].text).toBe("d");
    expect(windowed.offset).toBe(2);
    expect(windowed.total).toBe(4);
  });
});
