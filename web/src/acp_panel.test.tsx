import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AcpPanel, AcpPanelProps, AcpPanelView } from "./components/acp_panel";
import { AcpView } from "./acp";

const baseView: AcpView = {
  hasAcp: true,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  plan: null,
  commands: [],
  currentMode: null,
  sessionCapabilities: null,
  runStatus: null,
  thinkingStartTs: null,
};

const baseProps: AcpPanelProps = {
  acpView: baseView,
  subtitle: null,
  acpTab: "conversation",
  onSelectTab: () => {},
  showConversationBadge: false,
  showConversationJump: false,
  onJumpToConversationBottom: () => {},
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
    onJumpToPermissionHistory: () => {},
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
  },
};

function collectButtons(node: React.ReactNode, out: React.ReactElement[] = []): React.ReactElement[] {
  if (node == null || typeof node === "string" || typeof node === "number") {
    return out;
  }
  if (Array.isArray(node)) {
    for (const child of node) collectButtons(child, out);
    return out;
  }
  if (!React.isValidElement(node)) return out;
  if (node.type === "button") {
    out.push(node);
  }
  collectButtons((node.props as { children?: React.ReactNode }).children, out);
  return out;
}

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

  it("shows pending badge when conversation badge is enabled", () => {
    const html = renderToStaticMarkup(
      <AcpPanel
        {...baseProps}
        showConversationBadge={true}
        conversation={{ ...baseProps.conversation, pendingCount: 3 }}
      />
    );
    expect(html).toContain("+3");
  });

  it("invokes tab selection callbacks for both tabs", () => {
    const onSelectTab = vi.fn();
    const tree = AcpPanelView({
      ...baseProps,
      onSelectTab,
      showConversationBadge: true,
      conversation: { ...baseProps.conversation, pendingCount: 1 },
    });
    const buttons = collectButtons(tree);
    expect(buttons.length).toBeGreaterThanOrEqual(2);
    const conversationButton = buttons.find(
      (btn) =>
        typeof btn.props.className === "string" &&
        btn.props.className.includes("acp-tab-button")
    );
    const debugButton = buttons.find(
      (btn) =>
        typeof btn.props.className === "string" &&
        btn.props.className.includes("acp-tab-button") &&
        JSON.stringify(btn.props.children).includes("Debug")
    );
    expect(conversationButton).toBeDefined();
    expect(debugButton).toBeDefined();
    conversationButton?.props.onClick?.();
    debugButton?.props.onClick?.();
    expect(onSelectTab).toHaveBeenNthCalledWith(1, "conversation");
    expect(onSelectTab).toHaveBeenNthCalledWith(2, "debug");
  });

  it("renders debug view when acpTab is debug", () => {
    const html = renderToStaticMarkup(
      <AcpPanel {...baseProps} acpTab="debug" />
    );
    expect(html).toContain("Session");
    expect(html).toContain("Permissions");
    expect(html).toContain("Raw");
  });

  it("renders conversation jump button on ACP panel container layer", () => {
    const html = renderToStaticMarkup(
      <AcpPanel
        {...baseProps}
        showConversationJump={true}
      />
    );
    const panelPos = html.indexOf('class="acp relative');
    const conversationPos = html.indexOf('class="acp-conversation');
    const jumpPos = html.indexOf('class="acp-jump-bottom');
    expect(panelPos).toBeGreaterThanOrEqual(0);
    expect(conversationPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThan(conversationPos);
    expect(html).not.toContain("acp-conversation-jump-bottom");
    expect(html).toContain('aria-label="Jump to bottom"');
  });

  it("hides conversation jump button when debug tab is active", () => {
    const html = renderToStaticMarkup(
      <AcpPanel
        {...baseProps}
        acpTab="debug"
        showConversationJump={true}
      />
    );
    expect(html).not.toContain("acp-jump-bottom");
  });

  it("invokes jump callback when ACP jump button is clicked", () => {
    const onJumpToConversationBottom = vi.fn();
    const tree = AcpPanelView({
      ...baseProps,
      showConversationJump: true,
      onJumpToConversationBottom,
    });
    const buttons = collectButtons(tree);
    const jumpButton = buttons.find(
      (button) =>
        typeof button.props.className === "string" &&
        button.props.className.includes("acp-jump-bottom")
    );
    expect(jumpButton).toBeDefined();
    jumpButton?.props.onClick?.();
    expect(onJumpToConversationBottom).toHaveBeenCalledTimes(1);
  });
});
