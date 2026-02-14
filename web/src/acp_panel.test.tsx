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
  conversation: {
    items: [],
    windowOffset: 0,
    isFrozenView: false,
    shouldAutoCollapse: false,
    collapseCutoff: 0,
    runStatus: null,
    virtualTopSpacer: 0,
    virtualBottomSpacer: 0,
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

describe("AcpPanel layout", () => {
  it("renders subtitle and tabs in header", () => {
    const html = renderToStaticMarkup(
      <AcpPanel {...baseProps} subtitle="/repo/workdir" />
    );
    expect(html).toContain("/repo/workdir");
    expect(html).toContain("Conversation");
    expect(html).toContain("Debug");
    expect(html).not.toContain("Interrupt");
  });
});
