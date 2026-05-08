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

  it("extracts ACP config options and derives the current mode from them", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "config_option_update",
          config_options: [
            {
              id: "mode",
              label: "Mode",
              current_value: { type: "value_id", value: "danger_full_access" },
              select_options: [
                { value_id: "workspace_write", label: "Workspace Write" },
                { value_id: "danger_full_access", label: "Full Access" },
              ],
            },
            {
              id: "model",
              label: "Model",
              current_value: { type: "value_id", value: "gemini-2.5-pro" },
              select_options: [
                { value_id: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
                { value_id: "gpt-5", label: "GPT-5" },
              ],
            },
          ],
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.currentMode).toBe("danger_full_access");
    expect(view.configOptions).toHaveLength(2);
    expect(view.configOptions[0]).toMatchObject({
      id: "mode",
      currentValueId: "danger_full_access",
    });
    expect(view.configOptions[1]).toMatchObject({
      id: "model",
      currentValueId: "gemini-2.5-pro",
    });
  });

  it("clears ACP config selectors when the provider sends an explicit empty config_options list", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "config_option_update",
          config_options: [
            {
              id: "mode",
              label: "Mode",
              current_value: { type: "value_id", value: "workspace_write" },
              select_options: [
                { value_id: "workspace_write", label: "Workspace Write" },
              ],
            },
          ],
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "config_option_update",
          config_options: [],
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.configOptions).toEqual([]);
    expect(view.currentMode).toBeNull();
  });

  it("preserves existing ACP config selectors when config_options is missing", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "config_option_update",
          config_options: [
            {
              id: "mode",
              label: "Mode",
              current_value: { type: "value_id", value: "workspace_write" },
              select_options: [
                { value_id: "workspace_write", label: "Workspace Write" },
              ],
            },
          ],
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "config_option_update",
          current_mode_id: "danger_full_access",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.configOptions).toHaveLength(1);
    expect(view.configOptions[0]).toMatchObject({
      id: "mode",
      currentValueId: "workspace_write",
    });
    expect(view.currentMode).toBe("danger_full_access");
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

  it("merges tool call content updates and extracts text blocks", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call_update",
          id: "tool-1",
          content: [
            { type: "content", content: { type: "text", text: "{\"command\": \"g" } },
          ],
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call_update",
          id: "tool-1",
          content: [
            {
              type: "content",
              content: { type: "text", text: "{\"command\": \"grep\"}" },
            },
          ],
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.toolCalls.length).toBe(1);
    expect(view.toolCalls[0].content).toBe("{\"command\": \"grep\"}");
  });

  it("tracks terminal background activity on tool call updates", () => {
    const events = [
      {
        ts: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call_update",
          id: "tool-1",
          meta: {
            terminal_activity: {
              kind: "waited",
              command: "Run cargo test -p codex-core",
            },
          },
        }),
      },
      {
        ts: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call_update",
          id: "tool-1",
          meta: {
            terminal_activity: {
              kind: "interacted",
              command: "Run cargo test -p codex-core",
            },
          },
        }),
      },
    ];
    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].terminal_activities).toEqual([
      { kind: "waited", command: "Run cargo test -p codex-core" },
      { kind: "interacted", command: "Run cargo test -p codex-core" },
    ]);
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

  it("closes stale live tool calls once the same session has a later message", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-stale",
          title: "Shell",
          status: "in_progress",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "The command finished.",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("completed");
  });

  it("does not close live tool calls from messages in another session", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-live",
          title: "Shell",
          status: "in_progress",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        session_id: "s2",
        message: JSON.stringify({
          type: "agent_message",
          text: "Different session.",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("in_progress");
  });

  it("uses the latest message order per session when closing stale tool calls", () => {
    const events = [
      {
        ts: 30,
        event_id: 3,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Newest message first in the array.",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-stale",
          title: "Shell",
          status: "running",
        }),
      },
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Older message later in the array.",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("completed");
  });

  it("keeps live tool calls open when all same-session messages are older", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Older message.",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-live",
          title: "Shell",
          status: "pending",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("pending");
  });

  it("uses terminal run status before stale completion fallback", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-cancelled",
          title: "Shell",
          status: "running",
        }),
      },
      {
        ts: 15,
        event_id: 2,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "run_status",
          status: "cancelled",
        }),
      },
      {
        ts: 20,
        event_id: 3,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "agent_message",
          text: "Stopped.",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("cancelled");
  });

  it("does not apply terminal run status across sessions", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        session_id: "s1",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-live",
          title: "Shell",
          status: "running",
        }),
      },
      {
        ts: 15,
        event_id: 2,
        stream: "acp",
        session_id: "s2",
        message: JSON.stringify({
          type: "run_status",
          status: "cancelled",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.runStatus).toEqual({ status: "cancelled", session_id: "s2" });
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0].status).toBe("running");
  });

  it("applies terminal run status without a session id as a global status", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-global",
          title: "Shell",
          status: "RUNNING",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        message: JSON.stringify({
          type: "run_status",
          status: "interrupted",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.runStatus).toEqual({
      status: "interrupted",
      session_id: undefined,
    });
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0]).toMatchObject({
      id: "call-global",
      session_id: null,
      status: "interrupted",
    });
  });

  it("closes stale live tool calls without session ids from later sessionless messages", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-sessionless",
          title: "Shell",
          status: "in-progress",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "Finished without a session id.",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0]).toMatchObject({
      id: "call-sessionless",
      session_id: null,
      status: "completed",
    });
  });

  it("does not apply session-scoped terminal run status to sessionless tool calls", () => {
    const events = [
      {
        ts: 10,
        event_id: 1,
        stream: "acp",
        message: JSON.stringify({
          type: "tool_call",
          id: "call-sessionless-live",
          title: "Shell",
          status: "running",
        }),
      },
      {
        ts: 20,
        event_id: 2,
        stream: "acp",
        session_id: "s2",
        message: JSON.stringify({
          type: "run_status",
          status: "stopped",
        }),
      },
    ];

    const view = buildAcpView(events);
    expect(view.runStatus).toEqual({ status: "stopped", session_id: "s2" });
    expect(view.toolCalls).toHaveLength(1);
    expect(view.toolCalls[0]).toMatchObject({
      id: "call-sessionless-live",
      session_id: null,
      status: "running",
    });
  });
});
