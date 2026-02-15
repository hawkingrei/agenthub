import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { OutputBody } from "./components/output_body";
import { AcpPanelProps } from "./components/acp_panel";
import { AcpView } from "./acp";
import { OutputLine } from "./output_cache";

const baseAcpView: AcpView = {
  hasAcp: false,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

const makeAcpPanelProps = (override?: Partial<AcpView>): AcpPanelProps => ({
  acpView: { ...baseAcpView, ...override },
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
});

const renderBody = ({
  isOutputLoading,
  outputs,
  acpOverride,
}: {
  isOutputLoading: boolean;
  outputs: OutputLine[];
  acpOverride?: Partial<AcpView>;
}) =>
  renderToStaticMarkup(
    <OutputBody
      terminalRef={React.createRef<HTMLDivElement>()}
      isOutputLoading={isOutputLoading}
      outputs={outputs}
      ansi={(input) => input}
      acpPanelProps={makeAcpPanelProps(acpOverride)}
    />
  );

describe("OutputBody", () => {
  it("shows loading state before output is ready", () => {
    const html = renderBody({ isOutputLoading: true, outputs: [] });
    expect(html).toContain("Waiting for output");
    expect(html).not.toContain("No output yet");
  });

  it("renders ACP panel when ACP data exists", () => {
    const html = renderBody({
      isOutputLoading: false,
      outputs: [],
      acpOverride: { hasAcp: true },
    });
    expect(html).toContain("Conversation");
    expect(html).not.toContain("No output yet");
  });

  it("renders terminal output when lines exist", () => {
    const html = renderBody({
      isOutputLoading: false,
      outputs: [
        {
          event_id: 1,
          ts: 1,
          seq: "1",
          stream: "stdout",
          message: "hello",
          agent_id: "agent-1",
          session_id: "session-1",
        },
      ],
    });
    expect(html).toContain("hello");
    expect(html).not.toContain("No output yet");
  });

  it("shows empty state only when there is no output and no ACP data", () => {
    const html = renderBody({ isOutputLoading: false, outputs: [] });
    expect(html).toContain("No output yet");
  });
});
