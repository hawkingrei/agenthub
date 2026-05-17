import React, { Suspense, useMemo } from "react";
import { TeamWorkbenchContent, TeamPanelLoadingFallback } from "./team_workbench_content";
import type {
  TeamDefinitionRecord,
  TeamActorMessageRecord,
  AgentEvent,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
  TeamRunEventRecord,
  TeamMemberSnapshot,
  AgentRecord,
  AgentDiscoveryCardRecord,
} from "../../api";
import type { WorkspaceLens } from "../../app_route_selection";
import type { StepAction, TeamTab } from "./state";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type { MailboxTemplateKey, TeamMailboxChatActors } from "./mailbox_helpers";
import type { TeamRunStatusFilter } from "./run_helpers";
import { TeamConversationContainer } from "./TeamConversationContainer";
import { TeamTasksContainer } from "./TeamTasksContainer";
import { TeamThreadContainer } from "./TeamThreadContainer";
import {
  formatTs,
  toPrettyJson,
  HUMAN_MAILBOX_ACTOR_ID,
  type AgentWorkspaceStatusView,
  type TeamMemberAgentControlState,
  type TeamRuntimeControlTone,
} from "./page_helpers";
import {
  type TeamDebugTag,
  TeamDebugToolsHeader,
  TeamRunRequiredPanel,
  TeamRunOpsPanel,
} from "./team_debug_panels";
import { WorkspacePanelLoadingFallback } from "../../components/workspace_panel_loading_fallback";
import {
  buildTeamWorkspaceHeaderProps,
  buildTeamRunsPanelProps,
  buildTeamWorkbenchBodyProps,
} from "./team_page_route_props";
import { isAgentActiveStatus } from "../../agent_ws";

type TeamWorkspaceHeaderProps = Parameters<typeof buildTeamWorkspaceHeaderProps>[0];
type TeamRunsPanelProps = Parameters<typeof buildTeamRunsPanelProps>[0];

const LazyTeamMemberAcpPanel = React.lazy(async () => {
  const module = await import("../team_member_acp_panel");
  return { default: module.TeamMemberAcpPanel };
});

const LazyTeamStepsPanel = React.lazy(async () => {
  const module = await import("../team_steps_panel");
  return { default: module.TeamStepsPanel };
});

const LazyTeamMailboxPanel = React.lazy(async () => {
  const module = await import("../team_mailbox_panel");
  return { default: module.TeamMailboxPanel };
});

type TeamWorkbenchContainerProps = {
  // Shell & Layout
  showTeamBootstrapLoading: boolean;
  showTeamUnavailable: boolean;
  onBackToSelector: () => void;
  selectedTeam: TeamDefinitionRecord | null;
  isAgentWorkspace: boolean;
  teamSectionCardClassName: string;
  teamSectionTitleClassName: string;
  teamSectionBodyTextClassName: string;
  panelSecondaryButtonClassName: string;
  teamWorkbenchWorkspaceShellClassName: string;
  tab: TeamTab;
  activeWorkspaceLens: WorkspaceLens;
  developerMode: boolean;
  busy: string | null;

  // Header Inputs
  workspaceEyebrow: string | null;
  showDedicatedWorkspaceHeading: boolean;
  workspaceTitle: string;
  workspaceDescription: string | null;
  selectedAgentLabel: string;
  selectedAgentWorkspaceMemberId: string;
  selectedAgentStatusView: AgentWorkspaceStatusView;
  selectedAgentSpecDraft: TeamMemberProfileDraft | null;
  selectedAgentControlState: TeamMemberAgentControlState;
  showWorkspaceRuntimeBadge: boolean;
  selectedTeamRuntimeStatus: { label: string; online: number; total: number; status: string } | null | undefined;
  selectedTeamRuntimeControlTone: TeamRuntimeControlTone;
  workspaceAdvancedTabItems: TeamWorkspaceHeaderProps["workspaceAdvancedTabItems"];
  isAdvancedWorkspace: boolean;
  showRunActionsInAdvanced: boolean;
  canResumeActiveRun: boolean;
  canRestartActiveRun: boolean;
  workspaceDetailsOpen: boolean;
  workspaceDetailItems: TeamWorkspaceHeaderProps["workspaceDetailItems"];
  workspaceNoticeText: string | null;
  workspaceNoticeDotClassName: string;
  teamWorkbenchMutedButtonClassName: string;
  teamWorkbenchHeaderActionButtonClassName: string;
  workspaceToolbarClassName: string;
  workspaceToolbarButtonActiveClassName: string;
  workspaceToolbarButtonIdleClassName: string;
  workspaceNoticeClassName: string;
  workspaceNoticeTextClassName: string;
  teamRunMetaItemClassName: string;

  // Header Actions
  onTabChange: (tab: TeamTab) => void;
  onToggleWorkspaceDetails: () => void;
  onRefreshActiveRun: () => void;
  onCancelRun: () => void;
  onResumeRun: () => void;
  onRestartRun: () => void;
  onOpenTeamMemberEditModal: () => void;
  onStartSelectedTeamAgent: () => void;
  onStopSelectedTeamAgent: () => void;
  onDeleteSelectedTeamAgent: () => void;

  // Runs Panel Inputs
  onDeleteTeam: () => void;
  runStatusFilter: TeamRunStatusFilter;
  TEAM_RUN_STATUS_FILTER_OPTIONS: TeamRunsPanelProps["runStatusFilterOptions"];
  onRunStatusFilterChange: TeamRunsPanelProps["onRunStatusFilterChange"];
  onRefreshRuns: () => void;
  runsLoading: boolean;
  visibleRuns: TeamRunRecord[];
  activeRunIdForSelectedTeam: string | null;
  setActiveRunId: (id: string | null) => void;
  isActiveRunHiddenByFilter: boolean;
  activeRunForSelectedTeam: TeamRunRecord | null;
  totalLoadedRunsForTeam: number;
  runsHasMore: boolean;
  effectiveSelectedTeamId: string | null;
  onLoadMoreRuns: () => void;

  // Workbench Body Logic Props
  snapshot: TeamRunSnapshotRecord | null;
  snapshotLoading: boolean;
  onRefreshOverviewSnapshot: () => void;
  mailboxDisplayNameByActorId: Record<string, string>;
  selectedAgentWorkspaceSessionId: string | null;
  memberEvents: AgentEvent[];
  memberEventsLoading: boolean;
  memberEventsHasMore: boolean;
  onLoadOlderMemberConsole: () => void;
  onRefreshMemberConsole: () => void;
  teamDebugTag: TeamDebugTag;
  setTeamDebugTag: (tag: TeamDebugTag) => void;
  runContextId: string;
  setRunContextId: (id: string) => void;
  runInput: string;
  setRunInput: (input: string) => void;
  runLookupId: string;
  setRunLookupId: (id: string) => void;
  canCreateRun: boolean;
  runInputHasError: boolean;
  runInputValidation: { parsed: unknown; error: string | null };
  teamExecutionBlockedReason: string | null;
  onCreateRun: () => void;
  onLoadRunById: () => void;
  steps: TeamStepRecord[];
  onRefreshActiveRunSteps: () => void;
  stepKey: string;
  setStepKey: (key: string) => void;
  stepMemberId: string;
  onStepMemberIdChange: (id: string) => void;
  stepDependsOn: string;
  onStepDependsOnChange: (id: string) => void;
  stepInput: string;
  onStepInputChange: (input: string) => void;
  onSubmitStep: () => void;
  selectedStepId: string;
  setSelectedStepId: (id: string) => void;
  stepAction: StepAction;
  setStepAction: (action: StepAction) => void;
  stepRemoteTaskId: string;
  onStepRemoteTaskIdChange: (id: string) => void;
  stepOutput: string;
  onStepOutputChange: (output: string) => void;
  stepFailText: string;
  onStepFailTextChange: (text: string) => void;
  stepInputReason: string;
  onStepInputReasonChange: (reason: string) => void;
  stepInputRequiredPayload: string;
  onStepInputRequiredPayloadChange: (payload: string) => void;
  stepResumePayload: string;
  onStepResumePayloadChange: (payload: string) => void;
  onApplyStepAction: () => void;
  unreadByMemberId: Record<string, number>;
  chatActors: TeamMailboxChatActors;
  chatStickToBottom: boolean;
  chatMessagesRef: React.RefObject<HTMLUListElement | null>;
  onConversationScroll: () => void;
  onJumpConversationToBottom: () => void;
  conversationMessages: TeamActorMessageRecord[];
  onAcceptMessage: (message: TeamActorMessageRecord) => void | Promise<void>;
  onAcceptVisibleMessages: (messages: TeamActorMessageRecord[]) => void | Promise<void>;
  onSendChatMessage: () => void;
  MAILBOX_TEMPLATE_OPTIONS: Array<{ value: MailboxTemplateKey; label: string }>;
  onMailboxTemplateChange: (value: string) => void;
  onApplyMessageTemplate: () => void;
  onSendMessage: () => void;
  onRefreshInbox: () => void;
  selectedAgentWorkspaceSnapshot: TeamMemberSnapshot | null;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedAgentWorkspaceRuntimeMember: {
    role?: string | null;
    session_status?: string | null;
    agent_status?: string | null;
  } | null;
  selectedAgentWorkspaceAgent: Pick<AgentRecord, "status" | "target_node_id"> | null;
  oldestMemberEventId: number | null;
  onSendAgentAcpInput: (input: string, sessionId: string) => void;
  onCancelTeamMemberAcp: () => void;
  onSetTeamMemberAcpMode: (mode: string) => void;
  onSetTeamMemberAcpModel: (model: string) => void;
  onSetTeamMemberAcpConfig: (configId: string, value: string) => void;
  onForceNewTeamMemberSession: () => void;
  eventsLoading: boolean;
  oldestEventId: number | null;
  displayedRunEvents: TeamRunEventRecord[];
  previewMode: boolean;
  memberTargetNodeById: Record<string, string | null>;
  msgFromActorId: string;
  onMsgFromActorIdChange: (id: string) => void;
  msgToActorId: string;
  onMsgToActorIdChange: (id: string) => void;
  msgChannel: string;
  onMsgChannelChange: (id: string) => void;
  msgTransport: "local" | "remote";
  onMsgTransportChange: (val: "local" | "remote") => void;
  msgRoute: string;
  onMsgRouteChange: (val: string) => void;
  msgTemplate: string;
  msgPayload: string;
  onMsgPayloadChange: (val: string) => void;
  msgIdempotencyKey: string;
  onMsgIdempotencyKeyChange: (val: string) => void;
  inboxActorId: string;
  onInboxActorIdChange: (val: string) => void;
  inboxLimit: string;
  onInboxLimitChange: (val: string) => void;
  inboxAfterId: string;
  onInboxAfterIdChange: (val: string) => void;
  inboxIncludeDelivered: boolean;
  onInboxIncludeDeliveredChange: (val: boolean) => void;
  chatDraft: string;
  onChatDraftChange: (val: string) => void;

  // Additional props
  selectedTeamHasConfiguredMembers: boolean;
  selectedTeamDescription: string | null | undefined;
  teamMemberForgeLabel: string;
  teamMemberCopyExistingLabel: string;
  onOpenTeamMemberForge: () => void;
  onOpenTeamMemberCopyExisting: () => void;
  showRunContextLoading: boolean;
  showNoActiveRunNotice: boolean;
  onGoToRuns: () => void;
  selectedMemberId: string;
  setSelectedMemberId: React.Dispatch<React.SetStateAction<string>>;
  mailboxHasActiveRun: boolean;
  mailboxEmptyTitle: string;
  mailboxEmptyBody: string;
  eventsAutoRefresh: boolean;
  setEventsAutoRefresh: (val: boolean) => void;
  onRefreshEventsPanel: () => void;
  onLoadOlderEventsPanel: () => void;
  eventsHasMore: boolean;
  TEAM_EVENT_PREVIEW_LIMIT: number;
  selectedMemberDiscoveryCard: AgentDiscoveryCardRecord | null;
  selectedMemberDiscoveryCardLoading: boolean;
  onOpenMailboxForMember: (id: string) => void;
};

export const TeamWorkbenchContainer = React.memo(function TeamWorkbenchContainer(
  props: TeamWorkbenchContainerProps
) {
  const {
    showTeamBootstrapLoading,
    showTeamUnavailable,
    onBackToSelector,
    selectedTeam,
    isAgentWorkspace,
    teamSectionCardClassName,
    teamSectionTitleClassName,
    teamSectionBodyTextClassName,
    panelSecondaryButtonClassName,
    teamWorkbenchWorkspaceShellClassName,
    tab,
    activeWorkspaceLens,
    developerMode,
    busy,
    workspaceEyebrow,
    showDedicatedWorkspaceHeading,
    workspaceTitle,
    workspaceDescription,
    selectedAgentLabel,
    selectedAgentWorkspaceMemberId,
    selectedAgentStatusView,
    selectedAgentSpecDraft,
    selectedAgentControlState,
    showWorkspaceRuntimeBadge,
    selectedTeamRuntimeStatus,
    selectedTeamRuntimeControlTone,
    workspaceAdvancedTabItems,
    isAdvancedWorkspace,
    showRunActionsInAdvanced,
    canResumeActiveRun,
    canRestartActiveRun,
    workspaceDetailsOpen,
    workspaceDetailItems,
    workspaceNoticeText,
    workspaceNoticeDotClassName,
    teamWorkbenchMutedButtonClassName,
    teamWorkbenchHeaderActionButtonClassName,
    workspaceToolbarClassName,
    workspaceToolbarButtonActiveClassName,
    workspaceToolbarButtonIdleClassName,
    workspaceNoticeClassName,
    workspaceNoticeTextClassName,
    teamRunMetaItemClassName,
    onTabChange,
    onToggleWorkspaceDetails,
    onRefreshActiveRun,
    onCancelRun,
    onResumeRun,
    onRestartRun,
    onOpenTeamMemberEditModal,
    onStartSelectedTeamAgent,
    onStopSelectedTeamAgent,
    onDeleteSelectedTeamAgent,
    onDeleteTeam,
    runStatusFilter,
    TEAM_RUN_STATUS_FILTER_OPTIONS,
    onRunStatusFilterChange,
    onRefreshRuns,
    runsLoading,
    visibleRuns,
    activeRunIdForSelectedTeam,
    setActiveRunId,
    isActiveRunHiddenByFilter,
    activeRunForSelectedTeam,
    totalLoadedRunsForTeam,
    runsHasMore,
    effectiveSelectedTeamId,
    onLoadMoreRuns,
    snapshot,
    snapshotLoading,
    onRefreshOverviewSnapshot,
    mailboxDisplayNameByActorId,
    selectedAgentWorkspaceSessionId,
    memberEvents,
    memberEventsLoading,
    memberEventsHasMore,
    onLoadOlderMemberConsole,
    onRefreshMemberConsole,
    teamDebugTag,
    setTeamDebugTag,
    runContextId,
    setRunContextId,
    runInput,
    setRunInput,
    runLookupId,
    setRunLookupId,
    canCreateRun,
    runInputHasError,
    runInputValidation,
    teamExecutionBlockedReason,
    onCreateRun,
    onLoadRunById,
    steps,
    onRefreshActiveRunSteps,
    stepKey,
    setStepKey,
    stepMemberId,
    onStepMemberIdChange,
    stepDependsOn,
    onStepDependsOnChange,
    stepInput,
    onStepInputChange,
    onSubmitStep,
    selectedStepId,
    setSelectedStepId,
    stepAction,
    setStepAction,
    stepRemoteTaskId,
    onStepRemoteTaskIdChange,
    stepOutput,
    onStepOutputChange,
    stepFailText,
    onStepFailTextChange,
    stepInputReason,
    onStepInputReasonChange,
    stepInputRequiredPayload,
    onStepInputRequiredPayloadChange,
    stepResumePayload,
    onStepResumePayloadChange,
    onApplyStepAction,
    unreadByMemberId,
    chatActors,
    chatStickToBottom,
    chatMessagesRef,
    onConversationScroll,
    onJumpConversationToBottom,
    conversationMessages,
    onAcceptMessage,
    onAcceptVisibleMessages,
    onSendChatMessage,
    MAILBOX_TEMPLATE_OPTIONS,
    onMailboxTemplateChange,
    onApplyMessageTemplate,
    onSendMessage,
    onRefreshInbox,
    selectedAgentWorkspaceSnapshot,
    selectedMemberSnapshot,
    selectedAgentWorkspaceRuntimeMember,
    selectedAgentWorkspaceAgent,
    oldestMemberEventId,
    onSendAgentAcpInput,
    onCancelTeamMemberAcp,
    onSetTeamMemberAcpMode,
    onSetTeamMemberAcpModel,
    onSetTeamMemberAcpConfig,
    onForceNewTeamMemberSession,
    eventsLoading,
    oldestEventId,
    displayedRunEvents,
    previewMode,
    memberTargetNodeById,
    msgFromActorId,
    onMsgFromActorIdChange,
    msgToActorId,
    onMsgToActorIdChange,
    msgChannel,
    onMsgChannelChange,
    msgTransport,
    onMsgTransportChange,
    msgRoute,
    onMsgRouteChange,
    msgTemplate,
    msgPayload,
    onMsgPayloadChange,
    msgIdempotencyKey,
    onMsgIdempotencyKeyChange,
    inboxActorId,
    onInboxActorIdChange,
    inboxLimit,
    onInboxLimitChange,
    inboxAfterId,
    onInboxAfterIdChange,
    inboxIncludeDelivered,
    onInboxIncludeDeliveredChange,
    chatDraft,
    onChatDraftChange,
    selectedTeamHasConfiguredMembers,
    selectedTeamDescription,
    teamMemberForgeLabel,
    teamMemberCopyExistingLabel,
    onOpenTeamMemberForge,
    onOpenTeamMemberCopyExisting,
    showRunContextLoading,
    showNoActiveRunNotice,
    onGoToRuns,
    selectedMemberId,
    setSelectedMemberId,
    mailboxHasActiveRun,
    mailboxEmptyTitle,
    mailboxEmptyBody,
    eventsAutoRefresh,
    setEventsAutoRefresh,
    onRefreshEventsPanel,
    onLoadOlderEventsPanel,
    eventsHasMore,
    TEAM_EVENT_PREVIEW_LIMIT,
    selectedMemberDiscoveryCard,
    selectedMemberDiscoveryCardLoading,
    onOpenMailboxForMember,
  } = props;

  const workspaceHeaderProps = useMemo(
    () =>
      buildTeamWorkspaceHeaderProps({
        workspaceEyebrow,
        showDedicatedWorkspaceHeading,
        workspaceTitle,
        workspaceDescription,
        isAgentWorkspace,
        selectedAgentLabel,
        selectedAgentWorkspaceMemberId,
        selectedAgentStatusView,
        selectedAgentSpecDraft,
        selectedAgentControlState,
        showWorkspaceRuntimeBadge,
        selectedTeamRuntimeStatusLabel: selectedTeamRuntimeStatus?.label ?? "unknown",
        selectedTeamRuntimeOnline: selectedTeamRuntimeStatus?.online ?? 0,
        selectedTeamRuntimeTotal: selectedTeamRuntimeStatus?.total ?? 0,
        selectedTeamRuntimeControlTone,
        workspaceAdvancedTabItems,
        isAdvancedWorkspace,
        showRunActionsInAdvanced,
        activeRunStatus: activeRunForSelectedTeam?.status ?? null,
        canResumeActiveRun,
        canRestartActiveRun,
        developerMode,
        workspaceDetailsOpen,
        workspaceDetailItems,
        workspaceNoticeText,
        workspaceNoticeDotClassName,
        busy,
        chrome: {
          mutedButtonClassName: teamWorkbenchMutedButtonClassName,
          headerActionButtonClassName: teamWorkbenchHeaderActionButtonClassName,
          toolbarClassName: workspaceToolbarClassName,
          toolbarButtonActiveClassName: workspaceToolbarButtonActiveClassName,
          toolbarButtonIdleClassName: workspaceToolbarButtonIdleClassName,
          noticeClassName: workspaceNoticeClassName,
          noticeTextClassName: workspaceNoticeTextClassName,
          runMetaItemClassName: teamRunMetaItemClassName,
        },
        onTabChange,
        onToggleWorkspaceDetails,
        onRefreshActiveRun,
        onCancelRun,
        onResumeRun,
        onRestartRun,
        onOpenTeamMemberEditModal,
        onStartSelectedTeamAgent,
        onStopSelectedTeamAgent,
        onDeleteSelectedTeamAgent,
      }),
    [
      workspaceEyebrow,
      showDedicatedWorkspaceHeading,
      workspaceTitle,
      workspaceDescription,
      isAgentWorkspace,
      selectedAgentLabel,
      selectedAgentWorkspaceMemberId,
      selectedAgentStatusView,
      selectedAgentSpecDraft,
      selectedAgentControlState,
      showWorkspaceRuntimeBadge,
      selectedTeamRuntimeStatus?.label,
      selectedTeamRuntimeStatus?.online,
      selectedTeamRuntimeStatus?.total,
      selectedTeamRuntimeControlTone,
      workspaceAdvancedTabItems,
      isAdvancedWorkspace,
      showRunActionsInAdvanced,
      activeRunForSelectedTeam?.status,
      canResumeActiveRun,
      canRestartActiveRun,
      developerMode,
      workspaceDetailsOpen,
      workspaceDetailItems,
      workspaceNoticeText,
      workspaceNoticeDotClassName,
      busy,
      teamWorkbenchMutedButtonClassName,
      teamWorkbenchHeaderActionButtonClassName,
      workspaceToolbarClassName,
      workspaceToolbarButtonActiveClassName,
      workspaceToolbarButtonIdleClassName,
      workspaceNoticeClassName,
      workspaceNoticeTextClassName,
      teamRunMetaItemClassName,
      onTabChange,
      onToggleWorkspaceDetails,
      onRefreshActiveRun,
      onCancelRun,
      onResumeRun,
      onRestartRun,
      onOpenTeamMemberEditModal,
      onStartSelectedTeamAgent,
      onStopSelectedTeamAgent,
      onDeleteSelectedTeamAgent,
    ]
  );

  const runsPanelProps = useMemo(
    () =>
      buildTeamRunsPanelProps({
        selectedTeam: selectedTeam!,
        developerMode,
        busy,
        onDeleteTeam,
        onStartRun: onCreateRun,
        canStartRun: selectedTeamHasConfiguredMembers,
        runBlockedReason: teamExecutionBlockedReason,
        runStatusFilter,
        runStatusFilterOptions: TEAM_RUN_STATUS_FILTER_OPTIONS,
        onRunStatusFilterChange,
        onRefreshRuns,
        runsLoading,
        visibleRuns,
        activeRunId: activeRunIdForSelectedTeam,
        onActiveRunChange: setActiveRunId,
        isActiveRunHiddenByFilter,
        activeRun: activeRunForSelectedTeam,
        totalLoadedRunsForTeam,
        runsHasMore,
        selectedTeamId: effectiveSelectedTeamId,
        onLoadMoreRuns,
      }),
    [
      selectedTeam,
      developerMode,
      busy,
      onDeleteTeam,
      onCreateRun,
      selectedTeamHasConfiguredMembers,
      teamExecutionBlockedReason,
      runStatusFilter,
      TEAM_RUN_STATUS_FILTER_OPTIONS,
      onRunStatusFilterChange,
      onRefreshRuns,
      runsLoading,
      visibleRuns,
      activeRunIdForSelectedTeam,
      setActiveRunId,
      isActiveRunHiddenByFilter,
      activeRunForSelectedTeam,
      totalLoadedRunsForTeam,
      runsHasMore,
      effectiveSelectedTeamId,
      onLoadMoreRuns,
    ]
  );

  const overviewPanelProps = activeRunForSelectedTeam
    ? {
        snapshot,
        snapshotLoading,
        onRefreshSnapshot: onRefreshOverviewSnapshot,
        selectedMemberId,
        onOpenMailboxForMember,
        displayNameByActorId: mailboxDisplayNameByActorId,
        memberTargetNodeById,
      }
    : null;

  const eventsPanelProps = activeRunForSelectedTeam
    ? {
        eventsAutoRefresh,
        onEventsAutoRefreshChange: setEventsAutoRefresh,
        onRefreshEvents: onRefreshEventsPanel,
        onLoadOlderEvents: onLoadOlderEventsPanel,
        eventsLoading,
        previewMode,
        previewLimit: TEAM_EVENT_PREVIEW_LIMIT,
        eventsHasMore,
        oldestEventId,
        displayedRunEvents,
        formatTs,
        toPrettyJson,
      }
    : null;

  const stepsPanelProps = activeRunForSelectedTeam
    ? {
        developerMode,
        mode: "list_only" as const,
        steps,
        onRefreshSteps: onRefreshActiveRunSteps,
        stepKey,
        onStepKeyChange: setStepKey,
        stepMemberId,
        onStepMemberIdChange: onStepMemberIdChange,
        stepDependsOn,
        onStepDependsOnChange: onStepDependsOnChange,
        stepInput,
        onStepInputChange: onStepInputChange,
        onSubmitStep,
        busy,
        selectedStepId,
        onSelectedStepIdChange: setSelectedStepId,
        stepAction,
        onStepActionChange: setStepAction,
        stepRemoteTaskId,
        onStepRemoteTaskIdChange: onStepRemoteTaskIdChange,
        stepOutput,
        onStepOutputChange: onStepOutputChange,
        stepFailText,
        onStepFailTextChange: onStepFailTextChange,
        stepInputReason,
        onStepInputReasonChange: onStepInputReasonChange,
        stepInputRequiredPayload,
        onStepInputRequiredPayloadChange: onStepInputRequiredPayloadChange,
        stepResumePayload,
        onStepResumePayloadChange: onStepResumePayloadChange,
        onApplyStepAction,
      }
    : null;

  const mailboxPanelProps = activeRunForSelectedTeam
    ? {
        developerMode,
        mode: "full" as const,
        snapshot,
        humanActorId: HUMAN_MAILBOX_ACTOR_ID,
        selectedMemberId,
        unreadByMemberId,
        onSelectMember: setSelectedMemberId,
        chatActors,
        chatStickToBottom,
        chatMessagesRef,
        onConversationScroll,
        onJumpToBottom: onJumpConversationToBottom,
        conversationMessages,
        displayNameByActorId: mailboxDisplayNameByActorId,
        toPrettyJson,
        formatTs,
        busy,
        onAcceptMessage,
        onAcceptVisibleMessages,
        chatDraft,
        onChatDraftChange,
        onSendChatMessage,
        msgFromActorId,
        onMsgFromActorIdChange: onMsgFromActorIdChange,
        msgToActorId,
        onMsgToActorIdChange: onMsgToActorIdChange,
        msgChannel,
        onMsgChannelChange: onMsgChannelChange,
        msgTransport,
        onMsgTransportChange: onMsgTransportChange,
        msgRoute,
        onMsgRouteChange: onMsgRouteChange,
        mailboxTemplateOptions: MAILBOX_TEMPLATE_OPTIONS,
        msgTemplate,
        onMsgTemplateChange: onMailboxTemplateChange,
        onApplyMessageTemplate,
        msgPayload,
        onMsgPayloadChange: onMsgPayloadChange,
        msgIdempotencyKey,
        onMsgIdempotencyKeyChange: onMsgIdempotencyKeyChange,
        onSendMessage,
        inboxActorId,
        onInboxActorIdChange: onInboxActorIdChange,
        inboxLimit,
        onInboxLimitChange: onInboxLimitChange,
        inboxAfterId,
        onInboxAfterIdChange: onInboxAfterIdChange,
        inboxIncludeDelivered,
        onInboxIncludeDeliveredChange: onInboxIncludeDeliveredChange,
        onRefreshInbox,
      }
    : null;

  const memberConsolePanelProps = activeRunForSelectedTeam
    ? {
        snapshot,
        selectedMemberId,
        onSelectedMemberIdChange: setSelectedMemberId,
        selectedMemberSnapshot,
        selectedTargetNodeId: selectedAgentWorkspaceAgent
          ? selectedAgentWorkspaceAgent.target_node_id?.trim() || "main"
          : null,
        displayNameByActorId: mailboxDisplayNameByActorId,
        memberEvents,
        memberEventsHasMore,
        memberEventsLoading,
        eventsLoading,
        oldestMemberEventId,
        displayedRunEvents,
        previewLimit: TEAM_EVENT_PREVIEW_LIMIT,
        memberDiscoveryCard: selectedMemberDiscoveryCard,
        memberDiscoveryCardLoading: selectedMemberDiscoveryCardLoading,
        onRefresh: onRefreshMemberConsole,
        onLoadOlder: onLoadOlderMemberConsole,
        toPrettyJson,
        formatTs,
      }
    : null;

  const teamDebugChrome = useMemo(
    () => ({
      panelCardClassName: teamSectionCardClassName,
      sectionHeadingClassName: teamSectionTitleClassName,
      sectionBodyTextClassName: teamSectionBodyTextClassName,
      sectionHintTextClassName: "mt-2 text-[12px] leading-relaxed text-notion-text-muted",
      debugTabsClassName: "flex flex-wrap items-center gap-1 bg-notion-sidebar p-1 rounded-lg border border-notion-border",
      debugTabActiveClassName: "inline-flex items-center rounded-md bg-white px-3 py-1 text-[11px] font-bold uppercase tracking-wider text-notion-text shadow-sm",
      debugTabIdleClassName: "inline-flex items-center rounded-md px-3 py-1 text-[11px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:text-notion-text hover:bg-notion-hover",
      panelSecondaryButtonClassName,
    }),
    [
      teamSectionCardClassName,
      teamSectionTitleClassName,
      teamSectionBodyTextClassName,
      panelSecondaryButtonClassName,
    ]
  );

  const runOpsPanel = (
    <TeamRunOpsPanel
      chrome={teamDebugChrome}
      busy={busy}
      runContextId={runContextId}
      runInput={runInput}
      runLookupId={runLookupId}
      canCreateRun={canCreateRun}
      runInputHasError={runInputHasError}
      runInputError={runInputValidation.error}
      createRunTitle={runInputValidation.error ?? teamExecutionBlockedReason ?? "Create run"}
      parsedRunInput={runInputValidation.parsed}
      helperText={
        teamExecutionBlockedReason ??
        "Accepts any valid JSON value. Shortcut: Ctrl/Cmd + Enter to create run."
      }
      onRunContextIdChange={setRunContextId}
      onRunInputChange={setRunInput}
      onRunLookupIdChange={setRunLookupId}
      onCreateRun={onCreateRun}
      onLoadRunById={onLoadRunById}
      onUseExampleJson={() =>
        setRunInput(
          JSON.stringify(
            {
              task: "investigate",
              objective: "improve-team-run",
            },
            null,
            2
          )
        )
      }
      onSetEmptyObject={() => setRunInput("{}")}
      onFormatJson={() => {
        const parsed = runInputValidation.parsed;
        if (parsed === undefined && runInput.trim().length === 0) {
          setRunInput("{}");
          return;
        }
        if (runInputValidation.error || parsed === undefined) {
          return;
        }
        setRunInput(JSON.stringify(parsed, null, 2));
      }}
      onClearRunInput={() => setRunInput("")}
    />
  );

  const conversationPanel = <TeamConversationContainer />;

  const tasksPanel = <TeamTasksContainer />;

  const threadPane = <TeamThreadContainer />;

  const agentAcpPanel = (
    <Suspense
      fallback={
        <WorkspacePanelLoadingFallback
          className={teamSectionCardClassName}
          title="Loading agent ACP..."
          body="AgentHub is loading the selected member runtime context."
        />
      }
    >
      <LazyTeamMemberAcpPanel
        developerMode={developerMode}
        selectedMemberId={selectedAgentWorkspaceMemberId}
        memberTitle={selectedAgentLabel}
        hideMemberTitle={true}
        selectedMemberSnapshot={selectedAgentWorkspaceSnapshot}
        selectedMemberRole={
          selectedAgentWorkspaceRuntimeMember?.role ?? selectedAgentWorkspaceSnapshot?.role ?? null
        }
        selectedAgentStatus={selectedAgentWorkspaceAgent?.status ?? null}
        selectedTargetNodeId={
          selectedAgentWorkspaceAgent
            ? selectedAgentWorkspaceAgent.target_node_id?.trim() || "main"
            : null
        }
        selectedSessionId={selectedAgentWorkspaceSessionId}
        memberEvents={memberEvents}
        memberEventsHasMore={memberEventsHasMore}
        memberEventsLoading={memberEventsLoading}
        eventsLoading={eventsLoading}
        oldestMemberEventId={oldestMemberEventId}
        onSendInput={onSendAgentAcpInput}
        canControlAcp={isAgentActiveStatus(selectedAgentWorkspaceAgent?.status ?? null)}
        canInterrupt={isAgentActiveStatus(selectedAgentWorkspaceAgent?.status ?? null)}
        onInterrupt={onCancelTeamMemberAcp}
        onAcpSetMode={onSetTeamMemberAcpMode}
        onAcpSetModel={onSetTeamMemberAcpModel}
        onAcpSetConfig={onSetTeamMemberAcpConfig}
        onForceNewSession={onForceNewTeamMemberSession}
        onLoadOlder={onLoadOlderMemberConsole}
      />
    </Suspense>
  );

  const debugPanel = developerMode ? (
    <>
      <TeamDebugToolsHeader
        chrome={teamDebugChrome}
        teamDebugTag={teamDebugTag}
        onTeamDebugTagChange={setTeamDebugTag}
      />

      {teamDebugTag === "run_ops" && runOpsPanel}

      {teamDebugTag === "step_ops" && !activeRunForSelectedTeam && (
        <TeamRunRequiredPanel
          chrome={teamDebugChrome}
          title="Step Ops"
          body="Step operations require an active execution run. Start or select one in the Execution Runs tab first."
          onGoToRuns={onGoToRuns}
        />
      )}

      {teamDebugTag === "step_ops" && activeRunForSelectedTeam && (
        <Suspense fallback={<TeamPanelLoadingFallback />}>
          <LazyTeamStepsPanel
            developerMode={developerMode}
            mode="controls_only"
            steps={steps}
            onRefreshSteps={onRefreshActiveRunSteps}
            stepKey={stepKey}
            onStepKeyChange={setStepKey}
            stepMemberId={stepMemberId}
            onStepMemberIdChange={onStepMemberIdChange}
            stepDependsOn={stepDependsOn}
            onStepDependsOnChange={onStepDependsOnChange}
            stepInput={stepInput}
            onStepInputChange={onStepInputChange}
            onSubmitStep={onSubmitStep}
            busy={busy}
            selectedStepId={selectedStepId}
            onSelectedStepIdChange={setSelectedStepId}
            stepAction={stepAction}
            onStepActionChange={setStepAction}
            stepRemoteTaskId={stepRemoteTaskId}
            onStepRemoteTaskIdChange={onStepRemoteTaskIdChange}
            stepOutput={stepOutput}
            onStepOutputChange={onStepOutputChange}
            stepFailText={stepFailText}
            onStepFailTextChange={onStepFailTextChange}
            stepInputReason={stepInputReason}
            onStepInputReasonChange={onStepInputReasonChange}
            stepInputRequiredPayload={stepInputRequiredPayload}
            onStepInputRequiredPayloadChange={onStepInputRequiredPayloadChange}
            stepResumePayload={stepResumePayload}
            onStepResumePayloadChange={onStepResumePayloadChange}
            onApplyStepAction={onApplyStepAction}
          />
        </Suspense>
      )}

      {teamDebugTag === "mailbox_raw" && !activeRunForSelectedTeam && (
        <TeamRunRequiredPanel
          chrome={teamDebugChrome}
          title="Mailbox Raw"
          body="Mailbox raw operations require an active execution run. Start or select one in the Execution Runs tab first."
          onGoToRuns={onGoToRuns}
        />
      )}

      {teamDebugTag === "mailbox_raw" && activeRunForSelectedTeam && (
        <Suspense fallback={<TeamPanelLoadingFallback />}>
          <LazyTeamMailboxPanel
            developerMode={developerMode}
            mode="advanced_only"
            snapshot={snapshot}
            humanActorId={HUMAN_MAILBOX_ACTOR_ID}
            selectedMemberId={selectedMemberId}
            unreadByMemberId={unreadByMemberId}
            onSelectMember={setSelectedMemberId}
            chatActors={chatActors}
            chatStickToBottom={chatStickToBottom}
            chatMessagesRef={chatMessagesRef}
            onConversationScroll={onConversationScroll}
            onJumpToBottom={onJumpConversationToBottom}
            conversationMessages={conversationMessages}
            displayNameByActorId={mailboxDisplayNameByActorId}
            toPrettyJson={toPrettyJson}
            formatTs={formatTs}
            busy={busy}
            onAcceptMessage={onAcceptMessage}
            onAcceptVisibleMessages={onAcceptVisibleMessages}
            chatDraft={chatDraft}
            onChatDraftChange={onChatDraftChange}
            onSendChatMessage={onSendChatMessage}
            msgFromActorId={msgFromActorId}
            onMsgFromActorIdChange={onMsgFromActorIdChange}
            msgToActorId={msgToActorId}
            onMsgToActorIdChange={onMsgToActorIdChange}
            msgChannel={msgChannel}
            onMsgChannelChange={onMsgChannelChange}
            msgTransport={msgTransport}
            onMsgTransportChange={onMsgTransportChange}
            msgRoute={msgRoute}
            onMsgRouteChange={onMsgRouteChange}
            mailboxTemplateOptions={MAILBOX_TEMPLATE_OPTIONS}
            msgTemplate={msgTemplate}
            onMsgTemplateChange={onMailboxTemplateChange}
            onApplyMessageTemplate={onApplyMessageTemplate}
            msgPayload={msgPayload}
            onMsgPayloadChange={onMsgPayloadChange}
            msgIdempotencyKey={msgIdempotencyKey}
            onMsgIdempotencyKeyChange={onMsgIdempotencyKeyChange}
            onSendMessage={onSendMessage}
            inboxActorId={inboxActorId}
            onInboxActorIdChange={onInboxActorIdChange}
            inboxLimit={inboxLimit}
            onInboxLimitChange={onInboxLimitChange}
            inboxAfterId={inboxAfterId}
            onInboxAfterIdChange={onInboxAfterIdChange}
            inboxIncludeDelivered={inboxIncludeDelivered}
            onInboxIncludeDeliveredChange={onInboxIncludeDeliveredChange}
            onRefreshInbox={onRefreshInbox}
          />
        </Suspense>
      )}
    </>
  ) : null;

  const bodyProps = useMemo(
    () =>
      buildTeamWorkbenchBodyProps({
        conversationPanel,
        threadPane,
        tasksPanel,
        agentAcpPanel,
        overviewPanelProps,
        eventsPanelProps,
        stepsPanelProps,
        mailboxHasActiveRun,
        mailboxEmptyTitle,
        mailboxEmptyBody,
        onGoToRuns,
        mailboxPanelProps,
        memberConsolePanelProps,
        debugPanel,
      }),
    [
      conversationPanel,
      threadPane,
      tasksPanel,
      agentAcpPanel,
      overviewPanelProps,
      eventsPanelProps,
      stepsPanelProps,
      mailboxHasActiveRun,
      mailboxEmptyTitle,
      mailboxEmptyBody,
      onGoToRuns,
      mailboxPanelProps,
      memberConsolePanelProps,
      debugPanel,
    ]
  );

  return (
    <TeamWorkbenchContent
      showTeamBootstrapLoading={showTeamBootstrapLoading}
      showTeamUnavailable={showTeamUnavailable}
      onBackToSelector={onBackToSelector}
      selectedTeam={selectedTeam}
      isAgentWorkspace={isAgentWorkspace}
      teamSectionCardClassName={teamSectionCardClassName}
      teamSectionTitleClassName={teamSectionTitleClassName}
      teamSectionBodyTextClassName={teamSectionBodyTextClassName}
      panelSecondaryButtonClassName={panelSecondaryButtonClassName}
      teamWorkbenchWorkspaceShellClassName={teamWorkbenchWorkspaceShellClassName}
      workspaceHeaderProps={workspaceHeaderProps}
      selectedTeamHasConfiguredMembers={selectedTeamHasConfiguredMembers}
      selectedTeamDescription={selectedTeamDescription}
      teamMemberForgeLabel={teamMemberForgeLabel}
      teamMemberCopyExistingLabel={teamMemberCopyExistingLabel}
      onOpenTeamMemberForge={onOpenTeamMemberForge}
      onOpenTeamMemberCopyExisting={onOpenTeamMemberCopyExisting}
      tab={tab}
      runsPanelProps={runsPanelProps}
      showRunContextLoading={showRunContextLoading}
      showNoActiveRunNotice={showNoActiveRunNotice}
      activeWorkspaceLens={activeWorkspaceLens}
      {...bodyProps}
    />
  );
});
