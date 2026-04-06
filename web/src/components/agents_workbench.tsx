import React from "react";
import { AcpView } from "../acp";
import { AcpPermissionRecord, AgentRecord } from "../api";
import { OutputLine } from "../output_cache";
import {
  AcpConversationEventMeta,
  useAcpConversation,
} from "../hooks/use_acp_conversation";
import { getAcpConversationCacheStats } from "./acp_conversation_cache_stats";
import { resolveAcpInputDockConversationClearance } from "./acp_input_dock_clearance";
import { resolveInputDockJumpMode } from "./acp_panel_helpers";
import { InputDock } from "./input_dock";
import { OutputBody } from "./output_body";

const PERMISSION_JUMP_MAX_ATTEMPTS = 24;
const PERMISSION_JUMP_RETRY_DELAY_MS = 120;

type PendingPermissionJumpState = {
  toolCallId: string;
  sessionId: string | null;
  attempts: number;
};

type SendAcpInputOptions = {
  recordHistory?: boolean;
  clearComposer?: boolean;
};

type AgentsWorkbenchProps = {
  activeAgent: string;
  activeAgentRecord: AgentRecord | null;
  activeSessionId: string | null;
  developerMode: boolean;
  acpTab: "conversation" | "plan" | "debug";
  acpView: AcpView;
  eventMeta: Record<string, AcpConversationEventMeta>;
  isAgentActive: boolean;
  outputs: OutputLine[];
  terminalOutputs: OutputLine[];
  scopedAcpPermissionHistory: AcpPermissionRecord[];
  isOutputLoading: boolean;
  isConversationLoading: boolean;
  terminalRef: React.RefObject<HTMLDivElement>;
  input: string;
  inputHistory: string[];
  ansi: (input: string) => string;
  canControlAcp: boolean;
  canInterruptAcpRun: boolean;
  acpModeId: string;
  acpModelId: string;
  acpConfigId: string;
  acpConfigValue: string;
  isComposingRef: React.MutableRefObject<boolean>;
  onLoadOlderEvents: () => Promise<void>;
  onTerminalScroll: () => void;
  onSelectTab: (tab: "conversation" | "plan" | "debug") => void;
  onAcpModeIdChange: (value: string) => void;
  onAcpModelIdChange: (value: string) => void;
  onAcpConfigIdChange: (value: string) => void;
  onAcpConfigValueChange: (value: string) => void;
  onAcpSetMode: (value: string) => void;
  onAcpSetModel: (value: string) => void;
  onAcpSetConfig: () => void;
  onAcpCancel: () => void;
  onAcpClearSession: () => void;
  onInputChange: (value: string) => void;
  onSelectInputHistory: (value: string) => void;
  onNavigateInputHistory: (direction: "up" | "down") => void;
  onSendAcpInput: (
    rawText: string,
    options?: SendAcpInputOptions
  ) => Promise<void>;
  onJumpToTerminalBottom: () => void;
  showTerminalJump: boolean;
};

function shouldAttemptPermissionJump(
  pending: PendingPermissionJumpState | null,
  acpTab: "conversation" | "plan" | "debug",
  activeSessionId: string | null
): "idle" | "wait" | "attempt" | "clear" {
  if (!pending) return "idle";
  if (acpTab !== "conversation") return "wait";
  if (pending.sessionId && activeSessionId !== pending.sessionId) return "wait";
  if (pending.attempts >= PERMISSION_JUMP_MAX_ATTEMPTS) return "clear";
  return "attempt";
}

function AgentsWorkbenchView({
  activeAgent,
  activeAgentRecord,
  activeSessionId,
  developerMode,
  acpTab,
  acpView,
  eventMeta,
  isAgentActive,
  outputs,
  terminalOutputs,
  scopedAcpPermissionHistory,
  isOutputLoading,
  isConversationLoading,
  terminalRef,
  input,
  inputHistory,
  ansi,
  canControlAcp,
  canInterruptAcpRun,
  acpModeId,
  acpModelId,
  acpConfigId,
  acpConfigValue,
  isComposingRef,
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
  onInputChange,
  onSelectInputHistory,
  onNavigateInputHistory,
  onSendAcpInput,
  onJumpToTerminalBottom,
  showTerminalJump,
}: AgentsWorkbenchProps) {
  const [inputDockHeight, setInputDockHeight] = React.useState(0);
  const [pendingPermissionJump, setPendingPermissionJump] =
    React.useState<PendingPermissionJumpState | null>(null);

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

  React.useEffect(() => {
    if (!showInputDock) {
      setInputDockHeight(0);
    }
  }, [showInputDock]);

  const onJumpToPermissionHistory = React.useCallback(
    (permission: AcpPermissionRecord) => {
      const toolCallId = permission.tool_call_id?.trim();
      if (!toolCallId) return;
      onSelectTab("conversation");
      setPendingPermissionJump({
        toolCallId,
        sessionId: permission.session_id ?? null,
        attempts: 0,
      });
    },
    [onSelectTab]
  );

  React.useEffect(() => {
    const jumpDecision = shouldAttemptPermissionJump(
      pendingPermissionJump,
      acpTab,
      activeSessionId
    );
    if (jumpDecision === "idle" || jumpDecision === "wait") return;
    if (jumpDecision === "clear") {
      setPendingPermissionJump(null);
      return;
    }
    if (!pendingPermissionJump) return;
    if (jumpToConversationToolCall(pendingPermissionJump.toolCallId)) {
      setPendingPermissionJump(null);
      return;
    }
    const timer = window.setTimeout(() => {
      setPendingPermissionJump((previous) => {
        if (!previous) return previous;
        return { ...previous, attempts: previous.attempts + 1 };
      });
    }, PERMISSION_JUMP_RETRY_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [
    pendingPermissionJump,
    acpTab,
    activeSessionId,
    jumpToConversationToolCall,
  ]);

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
    return {
      totalConversationItems: acpConversation.conversationTotalItems,
      sourceConversationItems: acpConversation.conversationSourceItems,
      renderedConversationItems: acpConversation.conversationRenderedItems,
      pendingConversationItems: acpConversation.conversationPendingCount,
      virtualizedConversation: acpConversation.conversationVirtualized,
      stickToBottom: acpConversation.conversationStickToBottom,
      averageConversationHeight: Math.round(acpConversation.conversationAvgHeight),
      rawEventCount: acpView.rawEvents.length,
      toolCallCount: acpView.toolCalls.length,
      messageCount: acpView.messages.length,
      markdownCacheHits: cacheStats.markdownHits,
      markdownCacheMisses: cacheStats.markdownMisses,
      ansiCacheHits: cacheStats.ansiHits,
      ansiCacheMisses: cacheStats.ansiMisses,
      payloadParses: cacheStats.payloadParses,
      payloadParseFailures: cacheStats.payloadParseFailures,
    };
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

  return (
    <>
      <OutputBody
        terminalRef={terminalRef}
        onTerminalScroll={onTerminalScroll}
        isOutputLoading={isOutputLoading}
        isConversationLoading={isConversationLoading}
        outputs={outputs}
        ansi={ansi}
        acpPanelProps={acpPanelProps}
      />
      {showInputDock ? (
        <InputDock
          input={input}
          historyCommands={inputHistory}
          showInterrupt={acpView.hasAcp}
          canInterrupt={canInterruptAcpRun}
          onHeightChange={setInputDockHeight}
          onInputChange={onInputChange}
          onSendInput={onSendInput}
          onInterrupt={onAcpCancel}
          onNavigateHistory={onNavigateInputHistory}
          onSelectHistoryCommand={onSelectInputHistory}
          onJumpToBottom={inputDockJumpMode.onJumpToBottom}
          showConversationJump={inputDockJumpMode.showConversationJump}
          isComposingRef={isComposingRef}
        />
      ) : null}
    </>
  );
}

export const AgentsWorkbench = React.memo(AgentsWorkbenchView);
AgentsWorkbench.displayName = "AgentsWorkbench";
