import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpDebug, AcpDebugProps } from "./components/acp_debug";

const baseProps: AcpDebugProps = {
  terminalOutputs: [],
  ansi: (input) => input,
  terminalRef: React.createRef<HTMLDivElement>(),
  onTerminalScroll: () => {},
  showTerminalJump: false,
  onJumpToTerminalBottom: () => {},
  currentMode: "default",
  rawEvents: [],
  configOptions: [],
  acpPermissionHistory: [],
  acpModeId: "",
  acpModelId: "",
  acpConfigId: "",
  acpConfigValue: "",
  onAcpModeIdChange: () => {},
  onAcpModelIdChange: () => {},
  onAcpConfigIdChange: () => {},
  onAcpConfigValueChange: () => {},
  canControlAcp: false,
  onAcpSetMode: () => {},
  onAcpSetModel: () => {},
  onAcpSetConfig: () => {},
  onAcpCancel: () => {},
  onAcpClearSession: () => {},
  onJumpToPermissionHistory: () => {},
  runtimeMetrics: {
    totalConversationItems: 12,
    sourceConversationItems: 10,
    renderedConversationItems: 8,
    pendingConversationItems: 2,
    virtualizedConversation: false,
    stickToBottom: true,
    averageConversationHeight: 48,
    rawEventCount: 5,
    toolCallCount: 3,
    messageCount: 4,
    markdownCacheHits: 4,
    markdownCacheMisses: 1,
    ansiCacheHits: 6,
    ansiCacheMisses: 2,
    payloadParses: 10,
    payloadParseFailures: 1,
  },
};

describe("AcpDebug", () => {
  it("defaults to session controls view", () => {
    const html = renderToStaticMarkup(<AcpDebug {...baseProps} />);
    expect(html).toContain("Session Controls");
    expect(html).toContain("Mode ID");
    expect(html).toContain("Model ID");
    expect(html).toContain("Config ID");
    expect(html).not.toContain("<h4>Permissions</h4>");
    expect(html).not.toContain("<h4>Raw Events</h4>");
    expect(html).not.toContain("acp-raw");
  });

  it("renders debug tabs", () => {
    const html = renderToStaticMarkup(<AcpDebug {...baseProps} />);
    expect(html).toContain("Terminal");
    expect(html).toContain("Runtime");
    expect(html).toContain("Permissions");
    expect(html).toContain("Raw Events");
  });

  it("renders terminal tab with output", () => {
    const html = renderToStaticMarkup(
      <AcpDebug
        {...baseProps}
        initialTab="terminal"
        terminalOutputs={[
          {
            event_id: 1,
            ts: 1,
            seq: "1",
            stream: "stdout",
            message: "hello terminal",
            agent_id: "agent-1",
            session_id: "session-1",
          },
        ]}
      />
    );
    expect(html).toContain("Terminal");
    expect(html).toContain("hello terminal");
    expect(html).toContain("1 stdout");
    expect(html).toContain("0 stderr");
    expect(html).toContain("All");
    expect(html).toContain("Stderr");
    expect(html).toContain("System");
  });

  it("renders runtime metrics tab when initial tab is runtime", () => {
    const html = renderToStaticMarkup(
      <AcpDebug {...baseProps} initialTab="runtime" />
    );
    expect(html).toContain("Runtime Metrics");
    expect(html).toContain("12");
    expect(html).toContain("5");
    expect(html).toContain("80% hit");
    expect(html).toContain("75% hit");
    expect(html).toContain("9");
    expect(html).toContain("1 fail");
  });

  it("renders permissions tab with jump/copy controls", () => {
    const html = renderToStaticMarkup(
      <AcpDebug
        {...baseProps}
        initialTab="permissions"
        acpPermissionHistory={[
          {
            id: "perm-1",
            agent_id: "agent-1",
            session_id: "session-1",
            status: "responded",
            selected_option_id: "allow_once",
            options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
            created_at: 1,
            responded_at: 2,
            tool_call_id: "call-1",
            tool_call: { title: "Read file", raw_input: { path: "README.md" } },
          },
          {
            id: "perm-2",
            agent_id: "agent-1",
            session_id: "session-1",
            status: "timeout",
            options: [],
            created_at: 3,
            tool_call_id: "",
          },
        ]}
      />
    );
    expect(html).toContain("<h4>Permissions</h4>");
    expect(html).toContain("Read file");
    expect(html).toContain("Copy");
    expect(html).toContain("tool_call call-1");
    expect(html).toContain("no linked tool call in conversation");
  });

  it("renders raw events tab list when initial tab is raw", () => {
    const html = renderToStaticMarkup(
      <AcpDebug
        {...baseProps}
        initialTab="raw"
        rawEvents={[{ ts: 1, type: "agent_message", payload: { text: "hello" } }]}
      />
    );
    expect(html).toContain("Raw Events");
    expect(html).toContain("agent_message");
    expect(html).toContain("&quot;hello&quot;");
  });

  it("renders model and mode selectors from ACP config options", () => {
    const html = renderToStaticMarkup(
      <AcpDebug
        {...baseProps}
        configOptions={[
          {
            id: "mode",
            label: "Mode",
            currentValueId: "workspace_write",
            selectOptions: [
              { valueId: "workspace_write", label: "Workspace Write" },
              { valueId: "danger_full_access", label: "Full Access" },
            ],
          },
          {
            id: "model",
            label: "Model",
            currentValueId: "gemini-2.5-pro",
            selectOptions: [
              { valueId: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
              { valueId: "gpt-5", label: "GPT-5" },
            ],
          },
        ]}
      />
    );
    expect(html).toContain("Workspace Write");
    expect(html).toContain("Full Access");
    expect(html).toContain("Gemini 2.5 Pro");
    expect(html).toContain("GPT-5");
    expect(html).not.toContain('placeholder="Model ID"');
  });
});
