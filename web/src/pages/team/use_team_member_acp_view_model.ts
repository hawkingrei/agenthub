import React from "react";
import { buildAcpView } from "../../acp";
import type { AgentEvent, TeamMemberSnapshot } from "../../api";
import { getTeamStepRuntimeHandleId } from "../../api";
import { getAcpConversationCacheStats } from "../../components/acp_conversation_cache_stats";
import { resolveAcpInputDockConversationClearance } from "../../components/acp_input_dock_clearance";
import { resolveInputDockJumpMode } from "../../components/acp_panel_helpers";
import { useAcpConversation } from "../../hooks/use_acp_conversation";
import {
  ACP_INITIAL_VISIBLE_MESSAGE_TARGET,
  omitIncompleteLeadingAcpMessageEvents,
} from "./acp_history_prefetch";
import {
  peekTeamMemberAcpRenderCache,
  saveTeamMemberAcpRenderCache,
  touchTeamMemberAcpRenderCache,
} from "./team_member_acp_render_cache";

export type TeamMemberAcpTab = "conversation" | "plan" | "debug";

const ACTIVE_MEMBER_STATUSES = new Set([
  "running",
  "working",
  "submitted",
  "input_required",
  "pending",
]);

function normalizeStatusValue(status?: string | null): string {
  return status?.trim().toLowerCase() || "";
}

type UseTeamMemberAcpViewModelArgs = {
  developerMode: boolean;
  selectedMemberId: string;
  memberTitle?: string | null;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedMemberRole?: string | null;
  selectedSessionId?: string | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  oldestMemberEventId: number | null;
  acpTab: TeamMemberAcpTab;
  inputDockHeight: number;
  terminalShowJump: boolean;
  acpModeId: string;
  acpModelId: string;
  acpConfigId: string;
  acpConfigValue: string;
  canControlAcp?: boolean;
  canInterrupt?: boolean;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
  onInterrupt?: () => Promise<void> | void;
  onAcpSetMode?: (modeId: string) => Promise<void> | void;
  onAcpSetModel?: (modelId: string) => Promise<void> | void;
  onAcpSetConfig?: (configId: string, value: string) => Promise<void> | void;
  onForceNewSession?: () => Promise<void> | void;
  onLoadOlder?: () => Promise<void> | void;
  setAcpTab: React.Dispatch<React.SetStateAction<TeamMemberAcpTab>>;
  setAcpModeId: React.Dispatch<React.SetStateAction<string>>;
  setAcpModelId: React.Dispatch<React.SetStateAction<string>>;
  setAcpConfigId: React.Dispatch<React.SetStateAction<string>>;
  setAcpConfigValue: React.Dispatch<React.SetStateAction<string>>;
  ansi: (input: string) => string;
  terminalRef: React.RefObject<HTMLDivElement | null>;
  handleTerminalScroll: () => void;
  jumpToTerminalBottom: () => void;
  handleSubmitRequestUserInput: (text: string) => Promise<void>;
};

export function useTeamMemberAcpViewModel({
  developerMode,
  selectedMemberId,
  memberTitle: memberTitleProp,
  selectedMemberSnapshot,
  selectedMemberRole,
  selectedSessionId: selectedSessionIdProp,
  memberEvents,
  memberEventsHasMore,
  memberEventsLoading,
  oldestMemberEventId,
  acpTab,
  inputDockHeight,
  terminalShowJump,
  acpModeId,
  acpModelId,
  acpConfigId,
  acpConfigValue,
  canControlAcp = false,
  canInterrupt,
  onSendInput,
  onInterrupt,
  onAcpSetMode,
  onAcpSetModel,
  onAcpSetConfig,
  onForceNewSession,
  onLoadOlder,
  setAcpTab,
  setAcpModeId,
  setAcpModelId,
  setAcpConfigId,
  setAcpConfigValue,
  ansi,
  terminalRef,
  handleTerminalScroll,
  jumpToTerminalBottom,
  handleSubmitRequestUserInput,
}: UseTeamMemberAcpViewModelArgs) {
  const selectedSessionId =
    selectedSessionIdProp ?? getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step);
  const scopedMemberEvents = React.useMemo(() => {
    if (!selectedSessionId) {
      return memberEvents;
    }
    return memberEvents.filter((event) => (event.session_id ?? null) === selectedSessionId);
  }, [memberEvents, selectedSessionId]);
  const effectiveMemberEvents = React.useMemo(() => {
    if (scopedMemberEvents.length > 0) {
      return scopedMemberEvents;
    }
    if (!selectedSessionId) {
      return scopedMemberEvents;
    }
    if (!memberEventsLoading) {
      const cachedSessionEvents = peekTeamMemberAcpRenderCache(
        selectedMemberId,
        selectedSessionId
      );
      return cachedSessionEvents.length > 0 ? cachedSessionEvents : scopedMemberEvents;
    }
    return peekTeamMemberAcpRenderCache(selectedMemberId, selectedSessionId);
  }, [memberEventsLoading, scopedMemberEvents, selectedMemberId, selectedSessionId]);

  React.useEffect(() => {
    if (scopedMemberEvents.length > 0 || !memberEventsLoading) {
      return;
    }
    if (effectiveMemberEvents.length === 0) {
      return;
    }
    touchTeamMemberAcpRenderCache(selectedMemberId, selectedSessionId);
  }, [
    effectiveMemberEvents.length,
    scopedMemberEvents.length,
    memberEventsLoading,
    selectedMemberId,
    selectedSessionId,
  ]);

  React.useEffect(() => {
    if (scopedMemberEvents.length === 0) {
      return;
    }
    saveTeamMemberAcpRenderCache(selectedMemberId, selectedSessionId, scopedMemberEvents);
  }, [scopedMemberEvents, selectedMemberId, selectedSessionId]);

  const visibleMemberEvents = React.useMemo(
    () =>
      omitIncompleteLeadingAcpMessageEvents(
        effectiveMemberEvents,
        selectedSessionId ?? null
      ),
    [effectiveMemberEvents, selectedSessionId]
  );
  const acpEventLines = React.useMemo(
    () =>
      visibleMemberEvents.map((event) => ({
        ts: event.ts,
        seq: event.seq,
        event_id: event.event_id,
        stream: event.stream,
        message: event.message,
        session_id: event.session_id,
      })),
    [visibleMemberEvents]
  );
  const acpView = React.useMemo(() => buildAcpView(acpEventLines), [acpEventLines]);
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
        loaded: !memberEventsLoading || effectiveMemberEvents.length > 0,
      },
    };
  }, [
    effectiveMemberEvents.length,
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
      void onLoadOlder?.();
    },
  });
  const terminalOutputs = React.useMemo(
    () => effectiveMemberEvents.filter((event) => event.stream !== "acp"),
    [effectiveMemberEvents]
  );
  const hasVisibleConversationItems =
    acpConversation.conversationSourceItems >= ACP_INITIAL_VISIBLE_MESSAGE_TARGET;
  const hasRenderableConversationContent =
    acpConversation.conversationSourceItems > 0;
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
  const snapshotStatus =
    normalizeStatusValue(selectedMemberSnapshot?.status) ||
    normalizeStatusValue(selectedMemberSnapshot?.session_status);
  const acpRunStatus = normalizeStatusValue(acpView.runStatus?.status);
  const memberStatus =
    snapshotStatus || acpRunStatus || normalizeStatusValue(memberRoleLabel) || "unknown";
  const thinkingLabel =
    acpView.thinkingStartTs && ACTIVE_MEMBER_STATUSES.has(snapshotStatus || acpRunStatus)
    ? `thinking ${Math.max(0, Math.floor(Date.now() / 1000 - acpView.thinkingStartTs))}s`
    : null;
  const memberStatusLabel = thinkingLabel
    ? `${memberStatus} · ${thinkingLabel}`
    : memberStatus;
  const memberStatusClassToken = memberStatus.replace(/[^a-z0-9_-]+/g, "-");
  const developerTechnicalMetadata = React.useMemo(
    () => [{ label: "role", value: memberRoleLabel || "-" }],
    [memberRoleLabel]
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
      bottomAlignLatest: acpConversation.conversationShouldBottomAlignLatest,
      pendingCount: acpConversation.conversationPendingCount,
      avgHeight: acpConversation.conversationAvgHeight,
      topHint: memberEventsLoading && !hasRenderableConversationContent
        ? "Loading ACP events..."
        : acpConversation.showConversationTopReachedHint
          ? "Already at top"
          : null,
      focusedToolCallId: acpConversation.focusedConversationToolCallId,
      onScroll: acpConversation.handleConversationScroll,
      onWheel: acpConversation.handleConversationWheel,
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
      acpConversation.conversationShouldBottomAlignLatest,
      acpConversation.conversationVirtualBottomSpacer,
      acpConversation.conversationVirtualTopSpacer,
      acpConversation.conversationWindowOffset,
      acpConversation.focusedConversationToolCallId,
      acpConversation.handleConversationScroll,
      acpConversation.handleConversationWheel,
      acpConversation.isFrozenView,
      acpConversation.shouldAutoCollapse,
      acpConversation.showConversationTopReachedHint,
      acpView.runStatus?.status,
      ansi,
      canSendInput,
      handleSubmitRequestUserInput,
      hasRenderableConversationContent,
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
      return "Agent ACP · No active thread session yet";
    }
    if (memberEventsLoading && !hasRenderableConversationContent) {
      return "Agent ACP · Loading activity...";
    }
    if (!acpView.hasAcp && visibleMemberEvents.length === 0) {
      return "Agent ACP · Active thread has no events yet";
    }
    return "Agent ACP · Active thread";
  }, [
    acpView.hasAcp,
    hasRenderableConversationContent,
    memberEventsLoading,
    selectedMemberId,
    selectedSessionId,
    visibleMemberEvents.length,
  ]);
  const showInputDock = !(developerMode && effectiveAcpTab === "debug" && acpView.hasAcp);
  const hasVisibleInputDock = canSendInput && showInputDock;
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
        !hasVisibleConversationItems &&
        !hasRenderableConversationContent &&
        memberEventsLoading &&
        acpConversation.conversationSourceItems <
          ACP_INITIAL_VISIBLE_MESSAGE_TARGET,
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
        onJumpToPermissionHistory: () => {},
        runtimeMetrics: acpRuntimeMetrics,
      },
    }),
    [
      acpConfigId,
      acpConfigValue,
      acpConversation.jumpToConversationBottom,
      hasVisibleConversationItems,
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
      hasRenderableConversationContent,
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
      terminalOutputs,
      terminalShowJump,
      terminalRef,
      setAcpConfigId,
      setAcpConfigValue,
      setAcpModeId,
      setAcpModelId,
      setAcpTab,
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

  return {
    selectedSessionId,
    effectiveAcpTab,
    acpView,
    terminalOutputs,
    hasVisibleInputDock,
    canSendInput,
    canInterruptAcpRun,
    memberTitle,
    memberModelLabel,
    memberStatus,
    memberStatusLabel,
    memberStatusClassToken,
    panelSubtitle,
    developerTechnicalMetadata,
    acpPanelProps,
    inputDockJumpMode,
  };
}
