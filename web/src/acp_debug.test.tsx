import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpDebug, AcpDebugProps } from "./components/acp_debug";

const baseProps: AcpDebugProps = {
  currentMode: "default",
  rawEvents: [],
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
  runtimeMetrics: {
    totalConversationItems: 0,
    sourceConversationItems: 0,
    renderedConversationItems: 0,
    pendingConversationItems: 0,
    virtualizedConversation: false,
    stickToBottom: true,
    averageConversationHeight: 48,
    rawEventCount: 0,
    toolCallCount: 0,
    messageCount: 0,
    markdownCacheHits: 0,
    markdownCacheMisses: 0,
    ansiCacheHits: 0,
    ansiCacheMisses: 0,
    payloadParses: 0,
    payloadParseFailures: 0,
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
    expect(html).toContain("Runtime");
    expect(html).toContain("Permissions");
    expect(html).toContain("Raw Events");
  });
});
