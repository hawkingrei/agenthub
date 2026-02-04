import { describe, expect, it } from "vitest";
import { buildAcpView } from "./acp";

describe("buildAcpView", () => {
  it("does not merge messages across sessions", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({ type: "agent_message", text: "Hello" }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s2",
        message: JSON.stringify({ type: "agent_message", text: "World" }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(2);
    expect(view.messages[0].text).toBe("Hello");
    expect(view.messages[1].text).toBe("World");
  });

  it("merges messages within the same session and kind", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({ type: "agent_message", text: "Hello" }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({ type: "agent_message", text: " World" }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(1);
    expect(view.messages[0].text).toBe("Hello World");
  });
});
