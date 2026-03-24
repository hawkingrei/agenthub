import React from "react";
import { buildAcpView } from "../acp";
import { AgentEvent, TeamMemberSnapshot, getTeamStepRuntimeHandleId } from "../api";
import { AcpPanel } from "../components/acp_panel";
import { getAcpConversationCacheStats } from "../components/acp_conversation";
import { resolveInputDockJumpMode } from "../components/acp_panel_helpers";
import { InputDock } from "../components/input_dock";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import { pushInputHistory } from "../input_history";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberAcpPanelProps = {
  developerMode: boolean;
  selectedMemberId: string;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedMemberRole?: string | null;
  selectedSessionId?: string | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
  onForceNewSession?: () => Promise<void> | void;
  onRefresh: () => Promise<void> | void;
  onLoadOlder: () => Promise<void> | void;
};

type TeamMemberAcpTab = "conversation" | "plan" | "debug";

const NOOP = () => {};

export function TeamMemberAcpPanel(props: TeamMemberAcpPanelProps) {
  const {
    selectedMemberId,
    developerMode,
    selectedMemberSnapshot,
    selectedMemberRole,
    selectedSessionId: selectedSessionIdProp,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    eventsLoading,
    oldestMemberEventId,
    onSendInput,
    onForceNewSession,
    onRefresh,
    onLoadOlder,
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
  const [acpTab, setAcpTab] = React.useState<TeamMemberAcpTab>("conversation");
  const effectiveAcpTab = !developerMode && acpTab === "debug" ? "conversation" : acpTab;
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

  React.useEffect(() => {
    setAcpTab("conversation");
  }, [selectedMemberId, selectedSessionId]);
  React.useEffect(() => {
    setInput("");
    setInputHistory([]);
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, [selectedMemberId, selectedSessionId]);

  const canLoadOlder =
    Boolean(selectedMemberId.trim() && selectedSessionId) &&
    !memberEventsLoading &&
    memberEventsHasMore &&
    oldestMemberEventId != null;
  const canSendInput = Boolean(selectedMemberId.trim() && selectedSessionId && onSendInput);
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
  const handleSendInput = React.useCallback(async () => {
    const text = input.trim();
    if (!text || !selectedSessionId || !onSendInput || sendingInputRef.current) {
      return;
    }
    sendingInputRef.current = true;
    setSendingInput(true);
    try {
      await onSendInput(text, selectedSessionId);
      setInputHistory((prev) => pushInputHistory(prev, text));
      setInputHistoryCursor(-1);
      inputHistoryDraftRef.current = "";
      setInput("");
    } finally {
      sendingInputRef.current = false;
      setSendingInput(false);
    }
  }, [input, onSendInput, selectedSessionId]);
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
      return `session ${selectedSessionId} · loading`;
    }
    if (!acpView.hasAcp && memberEvents.length === 0) {
      return `session ${selectedSessionId} · no thread events yet`;
    }
    return `session ${selectedSessionId}`;
  }, [acpView.hasAcp, memberEvents.length, memberEventsLoading, selectedMemberId, selectedSessionId]);
  const hasSelectedMember = Boolean(selectedMemberId.trim());
  const canShowThreadOptions =
    hasSelectedMember || memberEventsLoading || memberEvents.length > 0;
  const acpPanelProps = React.useMemo(
    () => ({
      acpView,
      subtitle: panelSubtitle,
      mobileTitle: null,
      acpTab: effectiveAcpTab,
      developerMode,
      onSelectTab: (nextTab: TeamMemberAcpTab) => setAcpTab(nextTab),
      showConversationBadge: acpConversation.showConversationBadge,
      showConversationJump: acpConversation.showConversationJump,
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
        acpPermissionHistory: [],
        acpModeId,
        acpModelId,
        acpConfigId,
        acpConfigValue,
        onAcpModeIdChange: setAcpModeId,
        onAcpModelIdChange: setAcpModelId,
        onAcpConfigIdChange: setAcpConfigId,
        onAcpConfigValueChange: setAcpConfigValue,
        canControlAcp: false,
        onAcpSetMode: NOOP,
        onAcpSetModel: NOOP,
        onAcpSetConfig: NOOP,
        onAcpCancel: NOOP,
        onAcpClearSession: onForceNewSession ?? NOOP,
        acpClearSessionLabel: "Force New Session",
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
      acpConversationProps,
      acpModeId,
      acpModelId,
      acpRuntimeMetrics,
      acpView,
      ansi,
      developerMode,
      effectiveAcpTab,
      handleTerminalScroll,
      jumpToTerminalBottom,
      onForceNewSession,
      panelSubtitle,
      terminalOutputs,
      terminalShowJump,
    ]
  );
  const showInputDock = !(developerMode && effectiveAcpTab === "debug" && acpView.hasAcp);
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
    <div className={`${TEAM_PANEL_CARD_CLASS} flex min-h-0 flex-1 flex-col rounded-[12px] border-black/[0.06] p-2.5`}>
      {canShowThreadOptions && (
        <div className="flex shrink-0 flex-col gap-2">
          <div className={`${TEAM_PANEL_TOOLBAR_ACTIONS_CLASS} w-full justify-end gap-2`}>
            <button
              onClick={() => {
                void onRefresh();
              }}
              disabled={hasSelectedMember && selectedSessionId ? memberEventsLoading : eventsLoading}
              className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
              title="Refresh thread"
              aria-label="Refresh thread"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              <span>Refresh</span>
            </button>
            {canLoadOlder && (
              <button
                onClick={() => {
                  void onLoadOlder();
                }}
                disabled={!canLoadOlder}
                className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              >
                Load Older
              </button>
            )}
          </div>
          {developerMode && (
            <div className="mono flex flex-wrap items-center gap-2 text-xs text-ui-text-muted">
              <div className="rounded-lg border border-black/[0.06] bg-ui-surface/70 px-2.5 py-1.5">
                member={selectedMemberId || "-"}
              </div>
              <div className="rounded-lg border border-black/[0.06] bg-ui-surface/70 px-2.5 py-1.5">
                role={selectedMemberSnapshot?.role ?? selectedMemberRole ?? "-"}
              </div>
              <div className="rounded-lg border border-black/[0.06] bg-ui-surface/70 px-2.5 py-1.5">
                session={selectedSessionId ?? "-"}
              </div>
            </div>
          )}
        </div>
      )}

      {!selectedMemberId.trim() && (
        <p className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {shouldRenderPanel && (
        <div className="relative mt-2 min-h-0 flex-1">
          <AcpPanel {...acpPanelProps} />
        </div>
      )}

      {canSendInput && showInputDock && (
        <div className="mt-2.5 shrink-0">
          <InputDock
            input={input}
            historyCommands={inputHistory}
            showInterrupt={false}
            canInterrupt={false}
            sendDisabled={!canSendInput || sendingInput}
            onInputChange={handleInputChange}
            onSendInput={() => {
              void handleSendInput();
            }}
            onInterrupt={() => {}}
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
