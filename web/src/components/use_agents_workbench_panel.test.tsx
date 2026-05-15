// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AcpView } from "../acp";
import type { AgentRecord } from "../api";
import type { AgentsWorkbenchProps } from "./agents_workbench_types";
import { useAgentsWorkbenchPanel } from "./use_agents_workbench_panel";

const {
  useAcpConversationMock,
  useAgentsPermissionJumpMock,
  getAcpConversationCacheStatsMock,
} = vi.hoisted(() => ({
  useAcpConversationMock: vi.fn(),
  useAgentsPermissionJumpMock: vi.fn(),
  getAcpConversationCacheStatsMock: vi.fn(),
}));

vi.mock("../hooks/use_acp_conversation", () => ({
  useAcpConversation: useAcpConversationMock,
}));

vi.mock("./use_agents_permission_jump", () => ({
  useAgentsPermissionJump: useAgentsPermissionJumpMock,
}));

vi.mock("./acp_conversation_cache_stats", () => ({
  getAcpConversationCacheStats: getAcpConversationCacheStatsMock,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const baseView: AcpView = {
  hasAcp: true,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  configOptions: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

function createConversationState() {
  return {
    acpConversationRef: () => {},
    conversationRenderItems: [],
    conversationWindowOffset: 0,
    conversationVirtualTopSpacer: 0,
    conversationVirtualBottomSpacer: 0,
    isFrozenView: false,
    collapseCutoff: 0,
    shouldAutoCollapse: false,
    conversationStickToBottom: true,
    conversationShouldBottomAlignLatest: true,
    conversationPendingCount: 0,
    conversationAvgHeight: 48,
    conversationTotalItems: 6,
    conversationSourceItems: 6,
    conversationRenderedItems: 6,
    conversationVirtualized: false,
    conversationViewportUnderfilled: false,
    conversationNeedsViewportFill: false,
    focusedConversationToolCallId: null,
    showConversationJump: true,
    showConversationBadge: false,
    showConversationTopReachedHint: false,
    jumpToConversationBottom: vi.fn(),
    jumpToConversationToolCall: vi.fn(() => true),
    handleConversationScroll: vi.fn(),
  };
}

function createProps(
  override: Partial<AgentsWorkbenchProps> = {}
): AgentsWorkbenchProps {
  const activeAgentRecord: AgentRecord = {
    id: "agent-1",
    name: "Agent one",
    command: "agenthub",
    args: [],
    workdir: "/repo/workdir",
    status: "running",
    created_at: 1_775_491_200,
    updated_at: 1_775_491_200,
    code_mode: false,
    worktree_mode: "use_existing",
  };

  return {
    activeAgent: "agent-1",
    activeAgentRecord,
    activeSessionId: "session-1",
    developerMode: true,
    acpTab: "conversation",
    acpView: baseView,
    eventMeta: {},
    isAgentActive: true,
    outputs: [],
    terminalOutputs: [],
    scopedAcpPermissionHistory: [],
    isOutputLoading: false,
    isConversationLoading: false,
    terminalRef: React.createRef<HTMLDivElement>(),
    input: "hello",
    inputHistory: [],
    ansi: (input) => input,
    canControlAcp: true,
    canInterruptAcpRun: true,
    acpModeId: "",
    acpModelId: "",
    acpConfigId: "",
    acpConfigValue: "",
    isComposingRef: { current: false },
    onLoadOlderEvents: async () => {},
    onTerminalScroll: () => {},
    onSelectTab: () => {},
    onAcpModeIdChange: () => {},
    onAcpModelIdChange: () => {},
    onAcpConfigIdChange: () => {},
    onAcpConfigValueChange: () => {},
    onAcpSetMode: () => {},
    onAcpSetModel: () => {},
    onAcpSetConfig: () => {},
    onAcpCancel: () => {},
    onAcpClearSession: () => {},
    onInputChange: () => {},
    onSelectInputHistory: () => {},
    onNavigateInputHistory: () => {},
    onSendAcpInput: async () => {},
    onJumpToTerminalBottom: () => {},
    showTerminalJump: false,
    ...override,
  };
}

type HookResult = ReturnType<typeof useAgentsWorkbenchPanel>;

function HookHarness({
  params,
  onCapture,
}: {
  params: AgentsWorkbenchProps;
  onCapture: (result: HookResult) => void;
}) {
  const result = useAgentsWorkbenchPanel(params);

  useEffect(() => {
    onCapture(result);
  }, [result, onCapture]);

  return null;
}

describe("useAgentsWorkbenchPanel", () => {
  let container: HTMLDivElement;
  let root: Root;
  let latestResult: HookResult | null;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    latestResult = null;
    useAcpConversationMock.mockReset();
    useAcpConversationMock.mockReturnValue(createConversationState());
    useAgentsPermissionJumpMock.mockReset();
    useAgentsPermissionJumpMock.mockReturnValue({
      onJumpToPermissionHistory: vi.fn(),
    });
    getAcpConversationCacheStatsMock.mockReset();
    getAcpConversationCacheStatsMock.mockReturnValue({
      markdownHits: 3,
      markdownMisses: 1,
      ansiHits: 4,
      ansiMisses: 1,
      payloadParses: 5,
      payloadParseFailures: 1,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.restoreAllMocks();
  });

  async function renderHarness(params: AgentsWorkbenchProps) {
    await act(async () => {
      root.render(
        <HookHarness
          params={params}
          onCapture={(result) => {
            latestResult = result;
          }}
        />
      );
      await Promise.resolve();
    });
  }

  it("hides the input dock in ACP debug mode and clears stale dock height", async () => {
    await renderHarness(createProps());

    expect(latestResult?.showInputDock).toBe(true);

    await act(async () => {
      latestResult?.setInputDockHeight(96);
      await Promise.resolve();
    });

    expect(latestResult?.inputDockHeight).toBe(96);

    await renderHarness(
      createProps({
        developerMode: true,
        acpTab: "debug",
        acpView: { ...baseView, hasAcp: true },
      })
    );

    expect(latestResult?.showInputDock).toBe(false);
    expect(latestResult?.inputDockHeight).toBe(0);
    expect(latestResult?.acpPanelProps.showFloatingConversationJump).toBe(true);
    expect(latestResult?.acpPanelProps.conversationBottomClearance).toBe(0);
  });

  it("normalizes the debug tab for non-developer ACP views while keeping the dock", async () => {
    await renderHarness(
      createProps({
        developerMode: false,
        acpTab: "debug",
        acpView: { ...baseView, hasAcp: true },
      })
    );

    expect(latestResult?.showInputDock).toBe(true);
    expect(latestResult?.acpPanelProps.acpTab).toBe("conversation");
    expect(latestResult?.acpPanelProps.subtitle).toBe("/repo/workdir");
    expect(latestResult?.acpPanelProps.mobileTitle).toBe("Agent one");
    expect(latestResult?.acpPanelProps.conversation.bottomAlignLatest).toBe(true);
  });

  it("records composer history and clears the composer when sending input", async () => {
    const jumpToConversationBottom = vi.fn();
    useAcpConversationMock.mockReturnValue({
      ...createConversationState(),
      jumpToConversationBottom,
    });
    const onSendAcpInput = vi.fn().mockResolvedValue(undefined);

    await renderHarness(
      createProps({
        onSendAcpInput,
      })
    );

    await act(async () => {
      await latestResult?.onSendInput();
    });

    expect(jumpToConversationBottom).toHaveBeenCalledTimes(1);
    expect(onSendAcpInput).toHaveBeenCalledWith("hello", {
      recordHistory: true,
      clearComposer: true,
    });
  });

  it("forwards request-user-input submissions and ACP cancel gating", async () => {
    const jumpToConversationBottom = vi.fn();
    const onSendAcpInput = vi.fn().mockResolvedValue(undefined);
    useAcpConversationMock.mockReturnValue({
      ...createConversationState(),
      jumpToConversationBottom,
    });

    await renderHarness(
      createProps({
        canInterruptAcpRun: false,
        onSendAcpInput,
      })
    );

    const submitRequestUserInput =
      latestResult?.acpPanelProps.conversation.onSubmitRequestUserInput;
    expect(submitRequestUserInput).toBeTypeOf("function");

    await act(async () => {
      await submitRequestUserInput?.("approve");
    });

    expect(jumpToConversationBottom).toHaveBeenCalledTimes(1);
    expect(onSendAcpInput).toHaveBeenCalledWith("approve");
    expect(latestResult?.acpPanelProps.debug?.canCancelRun).toBe(false);
    expect(latestResult?.inputDockJumpMode.showConversationJump).toBe(true);
  });
});
