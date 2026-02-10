import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpPanel, AcpPanelProps } from "./components/acp_panel";
import { AcpView } from "./acp";

const baseView: AcpView = {
  hasAcp: true,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

const baseProps: AcpPanelProps = {
  acpView: baseView,
  subtitle: null,
  acpTab: "conversation",
  onSelectTab: () => {},
  showConversationBadge: false,
  canControlAcp: true,
  onAcpCancel: () => {},
  conversation: {
    items: [],
    windowOffset: 0,
    isFrozenView: false,
    shouldAutoCollapse: false,
    collapseCutoff: 0,
    stickToBottom: true,
    pendingCount: 0,
    avgHeight: 40,
    onScroll: () => {},
    containerRef: React.createRef<HTMLDivElement>(),
    ansi: (input) => input,
  },
  debug: {
    currentMode: null,
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
  },
};

const renderPanel = (override: Partial<AcpView>) =>
  renderToStaticMarkup(
    <AcpPanel {...baseProps} acpView={{ ...baseView, ...override }} />
  );

const interruptDisabled = (html: string) =>
  /acp-interrupt-button[^>]*disabled/.test(html);

describe("AcpPanel interrupt gating", () => {
  it("enables interrupt when a tool call is in progress without run status", () => {
    const html = renderPanel({
      toolCalls: [{ id: "call-1", title: "Tool", status: "in_progress" }],
    });
    expect(interruptDisabled(html)).toBe(false);
  });

  it("disables interrupt when run status is non-running and no tool call active", () => {
    const html = renderPanel({
      runStatus: { status: "completed" },
    });
    expect(interruptDisabled(html)).toBe(true);
  });
});

describe("AcpPanel layout", () => {
  it("renders subtitle and tabs in header", () => {
    const html = renderToStaticMarkup(
      <AcpPanel {...baseProps} subtitle="/repo/workdir" />
    );
    expect(html).toContain("/repo/workdir");
    expect(html).toContain("Conversation");
    expect(html).toContain("Debug");
  });
});
