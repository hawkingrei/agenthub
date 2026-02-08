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
        message: JSON.stringify({
          type: "agent_message",
          text: "Hello",
          chunk: true,
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: " World",
          chunk: true,
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(1);
    expect(view.messages[0].text).toBe("Hello World");
  });

  it("does not merge chunked agent messages onto a non-chunk message", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Final message",
          chunk: false,
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: " Streamed",
          chunk: true,
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(2);
    expect(view.messages[0].text).toBe("Final message");
    expect(view.messages[1].text).toBe(" Streamed");
  });

  it("does not merge non-chunk messages from the same session", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "user_message",
          text: "Hello",
          chunk: false,
          message_id: "m1",
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "user_message",
          text: "Again",
          chunk: false,
          message_id: "m2",
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(2);
    expect(view.messages[0].text).toBe("Hello");
    expect(view.messages[1].text).toBe("Again");
  });

  it("merges chunked agent_thought messages", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "Line 1",
          chunk: true,
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "\nLine 2",
          chunk: true,
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(1);
    expect(view.messages[0].text).toBe("Line 1\nLine 2");
  });

  it("orders chunked agent messages by chunk_index", () => {
    const events = [
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "World",
          chunk: true,
          message_id: "m1",
          chunk_index: 1,
        }),
      },
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Hello ",
          chunk: true,
          message_id: "m1",
          chunk_index: 0,
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.messages.length).toBe(1);
    expect(view.messages[0].text).toBe("Hello World");
  });

  it("tracks thinking timestamps while in thought and clears after message", () => {
    const thinkingEvents = [
      {
        ts: 10,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "Thinking...",
          chunk: false,
        }),
      },
      {
        ts: 12,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "More",
          chunk: true,
        }),
      },
    ];
    const viewWhileThinking = buildAcpView(thinkingEvents);
    expect(viewWhileThinking.thinkingStartTs).toBe(10);

    const finishedEvents = [
      ...thinkingEvents,
      {
        ts: 20,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Done",
          chunk: false,
        }),
      },
    ];
    const viewAfterMessage = buildAcpView(finishedEvents);
    expect(viewAfterMessage.thinkingStartTs).toBe(null);
  });

  it("clears thinking timestamp on tool calls", () => {
    const events = [
      {
        ts: 10,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "Thinking...",
          chunk: false,
        }),
      },
      {
        ts: 12,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-1",
          title: "Tool Call",
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.thinkingStartTs).toBe(null);
  });

  it("clears thinking timestamp on run status updates", () => {
    const events = [
      {
        ts: 10,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_thought",
          text: "Thinking...",
          chunk: false,
        }),
      },
      {
        ts: 12,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "run_status",
          status: "running",
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.thinkingStartTs).toBe(null);
    expect(view.runStatus?.status).toBe("running");
  });
});
