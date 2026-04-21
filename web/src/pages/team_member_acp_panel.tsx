import React from "react";
import { buildAcpView } from "../acp";
import { AgentEvent, TeamMemberSnapshot, getTeamStepRuntimeHandleId } from "../api";
import {
  AcpPanel,
} from "../components/acp_panel";
import { getAcpConversationCacheStats } from "../components/acp_conversation_cache_stats";
import { resolveAcpInputDockConversationClearance } from "../components/acp_input_dock_clearance";
import { OutputHeaderDetails } from "../components/output_header";
import { resolveInputDockJumpMode } from "../components/acp_panel_helpers";
import { InputDock } from "../components/input_dock";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import { pushInputHistory } from "../input_history";
import {
  OUTPUT_HEADER_META_CLASS,
  OUTPUT_HEADER_ROOT_CLASS,
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_HEADING_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
  OUTPUT_HEADER_TITLE_TEXT_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberAcpPanelProps = {
  developerMode: boolean;
  selectedMemberId: string;
  memberTitle?: string | null;
  hideMemberTitle?: boolean;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedMemberRole?: string | null;
  selectedSessionId?: string | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
  canControlAcp?: boolean;
  canInterrupt?: boolean;
  onInterrupt?: () => Promise<void> | void;
  onAcpSetMode?: (modeId: string) => Promise<void> | void;
  onAcpSetModel?: (modelId: string) => Promise<void> | void;
  onAcpSetConfig?: (configId: string, value: string) => Promise<void> | void;
  onForceNewSession?: () => Promise<void> | void;
  onLoadOlder?: () => Promise<void> | void;
};

type TeamMemberAcpTab = "conversation" | "plan" | "debug";
const TEAM_MEMBER_ACP_INITIAL_RENDER_MIN_EVENTS = 12;

const NOOP = () => {};

function hasIncompleteLeadingAcpMessage(
  events: AgentEvent[],
  sessionId: string | null | undefined
): boolean {
  const scopedSessionId = sessionId ?? null;
  const ordered = [...events].sort((left, right) => left.event_id - right.event_id);
  for (const event of ordered) {
    if (event.stream !== "acp") {
      continue;
    }
    if ((event.session_id ?? null) !== scopedSessionId) {
      continue;
    }
    const trimmed = event.message.trim();
    if (!trimmed.startsWith("{")) {
      continue;
    }
    try {
      const payload = JSON.parse(trimmed) as Record<string, unknown>;
      if (payload.type !== "agent_message") {
        continue;
      }
      if (payload.chunk !== true) {
        return false;
      }
      const chunkIndex =
        typeof payload.chunk_index === "number"
          ? payload.chunk_index
          : typeof payload.chunk_index === "string"
            ? Number.parseInt(payload.chunk_index, 10)
            : Number.NaN;
      return Number.isFinite(chunkIndex) && chunkIndex > 0;
    } catch {
      continue;
    }
  }
  return false;
}

function TeamMemberAcpPanelImpl(props: TeamMemberAcpPanelProps) {
  const {
    selectedMemberId,
    developerMode,
    selectedMemberSnapshot,
    memberTitle: memberTitleProp,
    hideMemberTitle = false,
    selectedMemberRole,
    selectedSessionId: selectedSessionIdProp,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    oldestMemberEventId,
    onSendInput,
    canControlAcp = false,
    canInterrupt,
    onInterrupt,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onForceNewSession,
    onLoadOlder = NOOP,
  } = props;

  const selectedSessionId =
    selectedSessionIdProp ?? getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step);
  const acpEventLines = React.useMemo(
    () =>
      memberEvents.map((event) => ({
        ts: event.ts,
        seq: event.seq,
        event_id: event.event_id,
        stream: event.stream,
        message: event.message,
        session_id: event.session_id,
      })),
    [memberEvents]
  );
  const acpView = React.useMemo(() => buildAcpView(acpEventLines), [acpEventLines]);
  const thinkingStartTs = acpView.thinkingStartTs;
  const [acpTab, setAcpTab] = React.useState<TeamMemberAcpTab>("conversation");
  const effectiveAcpTab = !developerMode && acpTab === "debug" ? "conversation" : acpTab;
  const [, setThinkingTick] = React.useState(0);
  const conversationEventMeta = React.useMemo(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId || !selectedSessionId) {
      return {};
    }
    return {
      [`${memberId}:${selectedSessionId}`]: {
        oldestId: oldestMemberEventId,
        hasMore: memberEventsHasMore,
        loading: memberEventsLoading,
        loaded: !memberEventsLoading || memberEvents.length > 0,
      },
    };
  }, [
    memberEvents.length,
    memberEventsHasMore,
    memberEventsLoading,
    oldestMemberEventId,
    selectedMemberId,
    selectedSessionId,
  ]);
  const acpConversation = useAcpConversation({
    acpView,
    activeAgent: selectedMemberId.trim() || null,
    activeSessionId: selectedSessionId ?? null,
    acpTab: effectiveAcpTab,
    eventMeta: conversationEventMeta,
    isAgentActive: Boolean(selectedSessionId),
    onLoadOlder: () => {
      void onLoadOlder();
    },
  });
  const terminalRef = React.useRef<HTMLDivElement | null>(null);
  const isComposingRef = React.useRef(false);
  const inputHistoryDraftRef = React.useRef("");
  const [input, setInput] = React.useState("");
  const [inputHistory, setInputHistory] = React.useState<string[]>([]);
  const [inputHistoryCursor, setInputHistoryCursor] = React.useState(-1);
  const [sendingInput, setSendingInput] = React.useState(false);
  const [terminalShowJump, setTerminalShowJump] = React.useState(false);
  const [inputDockHeight, setInputDockHeight] = React.useState(0);
  const [acpModeId, setAcpModeId] = React.useState("");
  const [acpModelId, setAcpModelId] = React.useState("");
  const [acpConfigId, setAcpConfigId] = React.useState("");
  const [acpConfigValue, setAcpConfigValue] = React.useState("");
  const sendingInputRef = React.useRef(false);
  const ansi = React.useCallback((inputValue: string) => inputValue, []);
  const terminalOutputs = React.useMemo(
    () => memberEvents.filter((event) => event.stream !== "acp"),
    [memberEvents]
  );
  const hasIncompleteLeadingConversationMessage = React.useMemo(
    () => hasIncompleteLeadingAcpMessage(memberEvents, selectedSessionId ?? null),
    [memberEvents, selectedSessionId]
  );

  React.useEffect(() => {
    setAcpTab("conversation");
  }, [selectedMemberId, selectedSessionId]);
  React.useEffect(() => {
    if (!thinkingStartTs) {
      return;
    }
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [thinkingStartTs]);
  React.useEffect(() => {
    setInput("");
    setInputHistory([]);
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, [selectedMemberId, selectedSessionId]);

  const canSendInput = Boolean(selectedMemberId.trim() && selectedSessionId && onSendInput);
  const hasInProgressToolCall = acpView.toolCalls.some(
    (call) => call.status === "in_progress"
  );
  const canInterruptAcpRun =
    Boolean(canInterrupt) &&
    (acpView.runStatus?.status === "running" || hasInProgressToolCall);
  const canSetMode = Boolean(canControlAcp && onAcpSetMode);
  const canSetModel = Boolean(canControlAcp && onAcpSetModel);
  const canSetConfig = Boolean(canControlAcp && onAcpSetConfig);
  const canCancelRun = Boolean(canControlAcp && onInterrupt && canInterruptAcpRun);
  const canClearSession = Boolean(onForceNewSession);
  const canControlAcpSession =
    canSetMode || canSetModel || canSetConfig || canCancelRun || canClearSession;
  const memberTitle = React.useMemo(() => {
    const explicitTitle = memberTitleProp?.trim();
    if (explicitTitle) {
      return explicitTitle;
    }
    const normalizedRole = (
      selectedMemberRole?.trim() ||
      selectedMemberSnapshot?.role?.trim() ||
      ""
    ).toLowerCase();
    if (normalizedRole === "leader") {
      return "Leader agent";
    }
    if (normalizedRole === "worker") {
      return "Worker agent";
    }
    return selectedMemberId.trim() ? "Selected agent" : "No team member selected";
  }, [memberTitleProp, selectedMemberId, selectedMemberRole, selectedMemberSnapshot?.role]);
  const memberModelLabel = selectedMemberSnapshot?.model?.trim() || null;
  const memberRoleLabel =
    selectedMemberRole?.trim() || selectedMemberSnapshot?.role?.trim() || null;
  const memberStatus = (
    acpView.runStatus?.status?.trim() ||
    selectedMemberSnapshot?.status?.trim() ||
    selectedMemberSnapshot?.session_status?.trim() ||
    memberRoleLabel ||
    "unknown"
  ).toLowerCase();
  const thinkingLabel = thinkingStartTs
    ? `thinking ${Math.max(0, Math.floor(Date.now() / 1000 - thinkingStartTs))}s`
    : null;
  const memberStatusLabel = thinkingLabel
    ? `${memberStatus} · ${thinkingLabel}`
    : memberStatus;
  const memberStatusClassToken = memberStatus.replace(/[^a-z0-9_-]+/g, "-");
  const developerTechnicalMetadata = React.useMemo(
    () => [{ label: "role", value: memberRoleLabel || "-" }],
    [memberRoleLabel]
  );
  const handleTerminalScroll = React.useCallback(() => {
    const element = terminalRef.current;
    if (!element) {
      setTerminalShowJump(false);
      return;
    }
    const remaining = element.scrollHeight - element.scrollTop - element.clientHeight;
    setTerminalShowJump(remaining > 48);
  }, []);
  const jumpToTerminalBottom = React.useCallback(() => {
    const element = terminalRef.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
    setTerminalShowJump(false);
  }, []);
  React.useEffect(() => {
    if (acpTab !== "debug") {
      return;
    }
    handleTerminalScroll();
  }, [acpTab, handleTerminalScroll, terminalOutputs.length]);
  const sendMemberInput = React.useCallback(async (
    rawText: string,
    options?: {
      recordHistory?: boolean;
      clearComposer?: boolean;
    }
  ) => {
    const text = rawText.trim();
    if (!text || !selectedSessionId || !onSendInput || sendingInputRef.current) {
      return;
    }
    sendingInputRef.current = true;
    setSendingInput(true);
    try {
      await onSendInput(text, selectedSessionId);
      if (options?.recordHistory) {
        setInputHistory((prev) => pushInputHistory(prev, text));
        setInputHistoryCursor(-1);
      }
      if (options?.clearComposer) {
        inputHistoryDraftRef.current = "";
        setInput("");
      }
    } finally {
      sendingInputRef.current = false;
      setSendingInput(false);
    }
  }, [onSendInput, selectedSessionId]);
  const handleSendInput = React.useCallback(async () => {
    await sendMemberInput(input, {
      recordHistory: true,
      clearComposer: true,
    });
  }, [input, sendMemberInput]);
  const handleSubmitRequestUserInput = React.useCallback(async (text: string) => {
    await sendMemberInput(text);
  }, [sendMemberInput]);
  const handleInputChange = React.useCallback(
    (value: string) => {
      setInput(value);
      if (inputHistoryCursor >= 0) {
        setInputHistoryCursor(-1);
      }
      inputHistoryDraftRef.current = value;
    },
    [inputHistoryCursor]
  );
  const handleNavigateHistory = React.useCallback(
    (direction: "up" | "down") => {
      if (inputHistory.length === 0) {
        return;
      }
      if (direction === "up") {
        if (inputHistoryCursor < 0) {
          inputHistoryDraftRef.current = input;
          setInputHistoryCursor(0);
          setInput(inputHistory[0] ?? "");
          return;
        }
        const nextCursor = Math.min(inputHistory.length - 1, inputHistoryCursor + 1);
        setInputHistoryCursor(nextCursor);
        setInput(inputHistory[nextCursor] ?? "");
        return;
      }
      if (inputHistoryCursor < 0) {
        return;
      }
      if (inputHistoryCursor === 0) {
        setInputHistoryCursor(-1);
        setInput(inputHistoryDraftRef.current);
        return;
      }
      const nextCursor = inputHistoryCursor - 1;
      setInputHistoryCursor(nextCursor);
      setInput(inputHistory[nextCursor] ?? "");
    },
    [input, inputHistory, inputHistoryCursor]
  );
  const handleSelectHistoryCommand = React.useCallback(
    (value: string) => {
      const nextCursor = inputHistory.findIndex((item) => item === value);
      setInputHistoryCursor(nextCursor);
      setInput(value);
      inputHistoryDraftRef.current = value;
    },
    [inputHistory]
  );
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
      topHint: memberEventsLoading
        ? "Loading ACP events..."
        : acpConversation.showConversationTopReachedHint
          ? "Already at top"
          : null,
      focusedToolCallId: acpConversation.focusedConversationToolCallId,
      onScroll: acpConversation.handleConversationScroll,
      containerRef: acpConversation.acpConversationRef,
      ansi,
      onSubmitRequestUserInput: canSendInput ? handleSubmitRequestUserInput : undefined,
    }),
    [
      acpConversation.acpConversationRef,
      acpConversation.collapseCutoff,
      acpConversation.conversationAvgHeight,
      acpConversation.conversationPendingCount,
      acpConversation.conversationRenderItems,
      acpConversation.conversationStickToBottom,
      acpConversation.conversationVirtualBottomSpacer,
      acpConversation.conversationVirtualTopSpacer,
      acpConversation.conversationWindowOffset,
      acpConversation.focusedConversationToolCallId,
      acpConversation.handleConversationScroll,
      acpConversation.isFrozenView,
      acpConversation.shouldAutoCollapse,
      acpConversation.showConversationTopReachedHint,
      acpView.runStatus?.status,
      ansi,
      canSendInput,
      handleSubmitRequestUserInput,
      memberEventsLoading,
    ]
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
    acpConversation.conversationAvgHeight,
    acpConversation.conversationPendingCount,
    acpConversation.conversationRenderedItems,
    acpConversation.conversationSourceItems,
    acpConversation.conversationStickToBottom,
    acpConversation.conversationTotalItems,
    acpConversation.conversationVirtualized,
    acpView.messages.length,
    acpView.rawEvents.length,
    acpView.toolCalls.length,
  ]);
  const panelSubtitle = React.useMemo(() => {
    if (!selectedMemberId.trim()) {
      return null;
    }
    if (!selectedSessionId) {
      return "No active thread session yet";
    }
    if (memberEventsLoading) {
      return "Active thread loading";
    }
    if (!acpView.hasAcp && memberEvents.length === 0) {
      return "Active thread has no events yet";
    }
    return "Active thread";
  }, [acpView.hasAcp, memberEvents.length, memberEventsLoading, selectedMemberId, selectedSessionId]);
  const showInputDock = !(developerMode && effectiveAcpTab === "debug" && acpView.hasAcp);
  const hasVisibleInputDock = canSendInput && showInputDock;
  React.useEffect(() => {
    if (!hasVisibleInputDock) {
      setInputDockHeight(0);
    }
  }, [hasVisibleInputDock]);
  const conversationBottomClearance = hasVisibleInputDock
    ? resolveAcpInputDockConversationClearance(inputDockHeight)
    : 0;
  const acpPanelProps = React.useMemo(
    () => ({
      acpView,
      subtitle: panelSubtitle,
      mobileTitle: null,
      acpTab: effectiveAcpTab,
      developerMode,
      conversationLoading:
        effectiveAcpTab === "conversation" &&
        ((memberEventsLoading &&
          acpConversation.conversationSourceItems <
            TEAM_MEMBER_ACP_INITIAL_RENDER_MIN_EVENTS) ||
          ((memberEventsLoading || memberEventsHasMore) &&
            hasIncompleteLeadingConversationMessage)),
      conversationBottomClearance,
      onSelectTab: (nextTab: TeamMemberAcpTab) => setAcpTab(nextTab),
      showConversationBadge: acpConversation.showConversationBadge,
      showConversationJump: acpConversation.showConversationJump,
      showFloatingConversationJump: !hasVisibleInputDock,
      onJumpToConversationBottom: acpConversation.jumpToConversationBottom,
      conversation: acpConversationProps,
      plan: {
        plan: acpView.plan,
      },
      debug: {
        terminalOutputs,
        ansi,
        terminalRef,
        onTerminalScroll: handleTerminalScroll,
        showTerminalJump: terminalShowJump,
        onJumpToTerminalBottom: jumpToTerminalBottom,
        currentMode: acpView.currentMode,
        rawEvents: acpView.rawEvents,
        configOptions: acpView.configOptions,
        acpPermissionHistory: [],
        acpModeId,
        acpModelId,
        acpConfigId,
        acpConfigValue,
        onAcpModeIdChange: setAcpModeId,
        onAcpModelIdChange: setAcpModelId,
        onAcpConfigIdChange: setAcpConfigId,
        onAcpConfigValueChange: setAcpConfigValue,
        canControlAcp: canControlAcpSession,
        canSetMode,
        canSetModel,
        canSetConfig,
        canCancelRun,
        canClearSession,
        onAcpSetMode: (value: string) => {
          void onAcpSetMode?.(value);
        },
        onAcpSetModel: (value: string) => {
          void onAcpSetModel?.(value);
        },
        onAcpSetConfig: () => {
          void onAcpSetConfig?.(acpConfigId, acpConfigValue);
        },
        onAcpCancel: () => {
          void onInterrupt?.();
        },
        onAcpClearSession: () => {
          void onForceNewSession?.();
        },
        acpClearSessionLabel: onForceNewSession ? "Force New Session" : undefined,
        onJumpToPermissionHistory: NOOP,
        runtimeMetrics: acpRuntimeMetrics,
      },
    }),
    [
      acpConfigId,
      acpConfigValue,
      acpConversation.jumpToConversationBottom,
      acpConversation.showConversationBadge,
      acpConversation.showConversationJump,
      acpConversation.conversationSourceItems,
      acpConversationProps,
      acpModeId,
      acpModelId,
      acpRuntimeMetrics,
      acpView,
      ansi,
      canCancelRun,
      canClearSession,
      canControlAcpSession,
      canSetConfig,
      canSetMode,
      canSetModel,
      developerMode,
      effectiveAcpTab,
      hasIncompleteLeadingConversationMessage,
      handleTerminalScroll,
      jumpToTerminalBottom,
      onAcpSetConfig,
      onAcpSetMode,
      onAcpSetModel,
      onForceNewSession,
      onInterrupt,
      panelSubtitle,
      conversationBottomClearance,
      hasVisibleInputDock,
      memberEventsLoading,
      memberEventsHasMore,
      terminalOutputs,
      terminalShowJump,
    ]
  );
  const inputDockJumpMode = React.useMemo(
    () =>
      resolveInputDockJumpMode({
        hasAcp: acpView.hasAcp,
        showConversationJump: acpConversation.showConversationJump,
        jumpToConversationBottom: acpConversation.jumpToConversationBottom,
        showTerminalJump: terminalShowJump,
        jumpToTerminalBottom,
      }),
    [
      acpConversation.jumpToConversationBottom,
      acpConversation.showConversationJump,
      acpView.hasAcp,
      jumpToTerminalBottom,
      terminalShowJump,
    ]
  );
  const shouldRenderPanel = Boolean(selectedMemberId.trim());

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-2.5`}>
      {!selectedMemberId.trim() && (
        <p className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {shouldRenderPanel && (
        <div className="relative mt-2 flex min-h-0 flex-1 flex-col gap-0 sm:gap-1.5">
          <div className={`${OUTPUT_HEADER_ROOT_CLASS} shrink-0`}>
            {!hideMemberTitle && (
              <div className={OUTPUT_HEADER_TITLE_CLASS}>
                <div className={OUTPUT_HEADER_TITLE_TEXT_CLASS}>
                  <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
                    <h2 className={OUTPUT_HEADER_TITLE_HEADING_CLASS}>{memberTitle}</h2>
                    {memberModelLabel ? (
                      <span className="agent-tag hidden sm:inline-flex">{memberModelLabel}</span>
                    ) : null}
                  </div>
                </div>
              </div>
            )}
            <div className={OUTPUT_HEADER_META_CLASS}>
              <StatusBadge
                label={memberStatusLabel}
                tone={resolveTeamRunStatusTone(memberStatus)}
                className={`agent-status status-${memberStatusClassToken}`}
                title={`status: ${memberStatusLabel}`}
              />
              {hideMemberTitle && memberModelLabel ? (
                <span className="agent-tag hidden sm:inline-flex">{memberModelLabel}</span>
              ) : null}
              <OutputHeaderDetails items={developerTechnicalMetadata} />
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <AcpPanel {...acpPanelProps} />
          </div>
        </div>
      )}

      {hasVisibleInputDock && (
        <div className="mt-1.5 shrink-0">
          <InputDock
            input={input}
            historyCommands={inputHistory}
            showInterrupt={Boolean(onInterrupt) && acpView.hasAcp}
            canInterrupt={canInterruptAcpRun}
            sendDisabled={!canSendInput || sendingInput}
            onHeightChange={setInputDockHeight}
            onInputChange={handleInputChange}
            onSendInput={() => {
              void handleSendInput();
            }}
            onInterrupt={() => {
              void onInterrupt?.();
            }}
            onNavigateHistory={handleNavigateHistory}
            onSelectHistoryCommand={handleSelectHistoryCommand}
            onJumpToBottom={inputDockJumpMode.onJumpToBottom}
            showConversationJump={inputDockJumpMode.showConversationJump}
            isComposingRef={isComposingRef}
          />
          {sendingInput && (
            <p className={`mt-1.5 ${TEAM_MUTED_TEXT_CLASS}`}>Sending prompt to selected agent...</p>
          )}
        </div>
      )}
    </div>
  );
}

export const TeamMemberAcpPanel = React.memo(TeamMemberAcpPanelImpl);
TeamMemberAcpPanel.displayName = "TeamMemberAcpPanel";
