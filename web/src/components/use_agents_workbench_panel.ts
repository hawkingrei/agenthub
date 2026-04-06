import React from "react";
import { AcpPanelProps } from "./acp_panel";
import { buildAcpRuntimeMetrics } from "./agents_workbench_metrics";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import { getAcpConversationCacheStats } from "./acp_conversation_cache_stats";
import { useAgentsPermissionJump } from "./use_agents_permission_jump";
import { resolveAcpInputDockConversationClearance } from "./acp_input_dock_clearance";
import { resolveInputDockJumpMode } from "./acp_panel_helpers";
import { AgentsWorkbenchProps } from "./agents_workbench_types";

type UseAgentsWorkbenchPanelResult = {
  acpPanelProps: AcpPanelProps;
  showInputDock: boolean;
  inputDockJumpMode: ReturnType<typeof resolveInputDockJumpMode>;
  inputDockHeight: number;
  setInputDockHeight: React.Dispatch<React.SetStateAction<number>>;
  onSendInput: () => Promise<void>;
};

export function useAgentsWorkbenchPanel({
  activeAgent,
  activeAgentRecord,
  activeSessionId,
  developerMode,
  acpTab,
  acpView,
  eventMeta,
  isAgentActive,
  terminalOutputs,
  scopedAcpPermissionHistory,
  terminalRef,
  input,
  ansi,
  canControlAcp,
  canInterruptAcpRun,
  acpModeId,
  acpModelId,
  acpConfigId,
  acpConfigValue,
  onLoadOlderEvents,
  onTerminalScroll,
  onSelectTab,
  onAcpModeIdChange,
  onAcpModelIdChange,
  onAcpConfigIdChange,
  onAcpConfigValueChange,
  onAcpSetMode,
  onAcpSetModel,
  onAcpSetConfig,
  onAcpCancel,
  onAcpClearSession,
  onSendAcpInput,
  onJumpToTerminalBottom,
  showTerminalJump,
}: AgentsWorkbenchProps): UseAgentsWorkbenchPanelResult {
  const [inputDockHeight, setInputDockHeight] = React.useState(0);

  const acpConversation = useAcpConversation({
    acpView,
    activeAgent,
    activeSessionId,
    acpTab,
    eventMeta,
    isAgentActive,
    onLoadOlder: onLoadOlderEvents,
  });
  const jumpToConversationBottom = acpConversation.jumpToConversationBottom;
  const jumpToConversationToolCall = acpConversation.jumpToConversationToolCall;

  const showInputDock = !(
    developerMode &&
    acpTab === "debug" &&
    acpView.hasAcp
  );

  const { onJumpToPermissionHistory } = useAgentsPermissionJump({
    acpTab,
    activeSessionId,
    jumpToConversationToolCall,
    onSelectTab,
  });

  React.useEffect(() => {
    if (!showInputDock) {
      setInputDockHeight(0);
    }
  }, [showInputDock]);

  const onSendInput = React.useCallback(async () => {
    jumpToConversationBottom();
    await onSendAcpInput(input, {
      recordHistory: true,
      clearComposer: true,
    });
  }, [jumpToConversationBottom, input, onSendAcpInput]);

  const onSubmitRequestUserInput = React.useCallback(
    async (text: string) => {
      jumpToConversationBottom();
      await onSendAcpInput(text);
    },
    [jumpToConversationBottom, onSendAcpInput]
  );

  const acpRuntimeMetrics = React.useMemo(() => {
    const cacheStats = getAcpConversationCacheStats();
    return buildAcpRuntimeMetrics({
      rawEventCount: acpView.rawEvents.length,
      toolCallCount: acpView.toolCalls.length,
      messageCount: acpView.messages.length,
      conversation: {
        totalItems: acpConversation.conversationTotalItems,
        sourceItems: acpConversation.conversationSourceItems,
        renderedItems: acpConversation.conversationRenderedItems,
        pendingItems: acpConversation.conversationPendingCount,
        virtualized: acpConversation.conversationVirtualized,
        stickToBottom: acpConversation.conversationStickToBottom,
        averageHeight: acpConversation.conversationAvgHeight,
      },
      cacheStats,
    });
  }, [
    acpConversation.conversationTotalItems,
    acpConversation.conversationSourceItems,
    acpConversation.conversationRenderedItems,
    acpConversation.conversationPendingCount,
    acpConversation.conversationVirtualized,
    acpConversation.conversationStickToBottom,
    acpConversation.conversationAvgHeight,
    acpView.rawEvents.length,
    acpView.toolCalls.length,
    acpView.messages.length,
  ]);

  const acpConversationProps = React.useMemo(
    () => ({
      items: acpConversation.conversationRenderItems,
      windowOffset: acpConversation.conversationWindowOffset,
      isFrozenView: acpConversation.isFrozenView,
      shouldAutoCollapse: acpConversation.shouldAutoCollapse,
      collapseCutoff: acpConversation.collapseCutoff,
      runStatus: acpView.runStatus?.status ?? null,
      virtualTopSpacer: acpConversation.conversationVirtualTopSpacer,
      virtualBottomSpacer: acpConversation.conversationVirtualBottomSpacer,
      stickToBottom: acpConversation.conversationStickToBottom,
      pendingCount: acpConversation.conversationPendingCount,
      avgHeight: acpConversation.conversationAvgHeight,
      topHint: acpConversation.showConversationTopReachedHint
        ? "Already at top"
        : null,
      focusedToolCallId: acpConversation.focusedConversationToolCallId,
      onScroll: acpConversation.handleConversationScroll,
      containerRef: acpConversation.acpConversationRef,
      ansi,
      onSubmitRequestUserInput,
    }),
    [
      acpConversation.conversationRenderItems,
      acpConversation.conversationWindowOffset,
      acpConversation.isFrozenView,
      acpConversation.shouldAutoCollapse,
      acpConversation.collapseCutoff,
      acpConversation.conversationVirtualTopSpacer,
      acpConversation.conversationVirtualBottomSpacer,
      acpConversation.conversationStickToBottom,
      acpConversation.conversationPendingCount,
      acpConversation.conversationAvgHeight,
      acpConversation.showConversationTopReachedHint,
      acpConversation.focusedConversationToolCallId,
      acpConversation.handleConversationScroll,
      acpConversation.acpConversationRef,
      acpView.runStatus?.status,
      ansi,
      onSubmitRequestUserInput,
    ]
  );

  const acpDebugProps = React.useMemo(
    () => ({
      terminalOutputs,
      ansi,
      terminalRef,
      onTerminalScroll,
      showTerminalJump,
      onJumpToTerminalBottom,
      currentMode: acpView.currentMode,
      rawEvents: acpView.rawEvents,
      configOptions: acpView.configOptions,
      acpPermissionHistory: scopedAcpPermissionHistory,
      acpModeId,
      acpModelId,
      acpConfigId,
      acpConfigValue,
      onAcpModeIdChange,
      onAcpModelIdChange,
      onAcpConfigIdChange,
      onAcpConfigValueChange,
      canControlAcp,
      canCancelRun: canInterruptAcpRun,
      onAcpSetMode,
      onAcpSetModel,
      onAcpSetConfig,
      onAcpCancel,
      onAcpClearSession,
      onJumpToPermissionHistory,
      runtimeMetrics: acpRuntimeMetrics,
    }),
    [
      terminalOutputs,
      ansi,
      terminalRef,
      onTerminalScroll,
      showTerminalJump,
      onJumpToTerminalBottom,
      acpView.currentMode,
      acpView.rawEvents,
      acpView.configOptions,
      scopedAcpPermissionHistory,
      acpModeId,
      acpModelId,
      acpConfigId,
      acpConfigValue,
      onAcpModeIdChange,
      onAcpModelIdChange,
      onAcpConfigIdChange,
      onAcpConfigValueChange,
      canControlAcp,
      canInterruptAcpRun,
      onAcpSetMode,
      onAcpSetModel,
      onAcpSetConfig,
      onAcpCancel,
      onAcpClearSession,
      onJumpToPermissionHistory,
      acpRuntimeMetrics,
    ]
  );

  const conversationBottomClearance = showInputDock
    ? resolveAcpInputDockConversationClearance(inputDockHeight)
    : 0;

  const acpPanelProps = React.useMemo(
    () => ({
      acpView,
      subtitle: activeAgentRecord?.workdir ?? null,
      mobileTitle: activeAgentRecord?.name ?? null,
      acpTab: !developerMode && acpTab === "debug" ? "conversation" : acpTab,
      developerMode,
      conversationBottomClearance,
      onSelectTab,
      showConversationBadge: acpConversation.showConversationBadge,
      showConversationJump: acpConversation.showConversationJump,
      showFloatingConversationJump: !showInputDock,
      onJumpToConversationBottom: jumpToConversationBottom,
      conversation: acpConversationProps,
      plan: {
        plan: acpView.plan,
      },
      debug: acpDebugProps,
    }),
    [
      acpView,
      activeAgentRecord?.workdir,
      activeAgentRecord?.name,
      developerMode,
      acpTab,
      conversationBottomClearance,
      onSelectTab,
      acpConversation.showConversationBadge,
      acpConversation.showConversationJump,
      jumpToConversationBottom,
      showInputDock,
      acpConversationProps,
      acpDebugProps,
    ]
  );

  const inputDockJumpMode = React.useMemo(
    () =>
      resolveInputDockJumpMode({
        hasAcp: acpView.hasAcp,
        showConversationJump: acpConversation.showConversationJump,
        jumpToConversationBottom,
        showTerminalJump,
        jumpToTerminalBottom: onJumpToTerminalBottom,
      }),
    [
      acpView.hasAcp,
      acpConversation.showConversationJump,
      jumpToConversationBottom,
      showTerminalJump,
      onJumpToTerminalBottom,
    ]
  );

  return {
    acpPanelProps,
    showInputDock,
    inputDockJumpMode,
    inputDockHeight,
    setInputDockHeight,
    onSendInput,
  };
}
