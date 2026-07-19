// @vitest-environment jsdom
import React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TeamWorkspaceProvider, type TeamWorkspaceContextValue } from "./team_workspace_context";
import { TeamWorkbenchContainer, type TeamWorkbenchRuntimeContext } from "./TeamWorkbenchContainer";

vi.mock("./team_workbench_content", () => ({
  TeamPanelLoadingFallback: () => <div data-testid="team-panel-loading" />,
  TeamWorkbenchContent: ({ debugPanel }: { debugPanel?: React.ReactNode }) => (
    <div data-testid="team-workbench-content">{debugPanel}</div>
  ),
}));

vi.mock("./team_debug_panels", () => ({
  TeamDebugToolsHeader: () => <div data-testid="team-debug-tools-header" />,
  TeamRunRequiredPanel: () => <div data-testid="team-run-required-panel" />,
  TeamRunOpsPanel: ({
    onUseExampleJson,
    onSetEmptyObject,
    onFormatJson,
    onClearRunInput,
    runInput,
  }: {
    onUseExampleJson: () => void;
    onSetEmptyObject: () => void;
    onFormatJson: () => void;
    onClearRunInput: () => void;
    runInput: string;
  }) => (
    <div>
      <pre data-testid="run-input">{runInput}</pre>
      <button type="button" onClick={onUseExampleJson}>
        Example
      </button>
      <button type="button" onClick={onSetEmptyObject}>
        Empty object
      </button>
      <button type="button" onClick={onFormatJson}>
        Format
      </button>
      <button type="button" onClick={onClearRunInput}>
        Clear
      </button>
    </div>
  ),
}));

vi.mock("./TeamConversationContainer", () => ({
  TeamConversationContainer: () => <div data-testid="team-conversation" />,
}));

vi.mock("./TeamTasksContainer", () => ({
  TeamTasksContainer: () => <div data-testid="team-tasks" />,
}));

function findButton(container: HTMLElement, label: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find((node) =>
    node.textContent?.includes(label)
  );
  if (!button) {
    throw new Error(`button not found: ${label}`);
  }
  return button as HTMLButtonElement;
}

function buildRuntimeContext(runInput: string, setRunInput: (value: string) => void): TeamWorkbenchRuntimeContext {
  const activeRun = {
    id: "run-1",
    team_id: "team-1",
    context_id: "ctx-1",
    status: "working" as const,
    input: {},
    created_at: 1,
    started_at: null,
    ended_at: null,
  };
  let parsed: unknown;
  let error: string | null = null;
  try {
    parsed = runInput.trim() ? JSON.parse(runInput) : undefined;
  } catch (err) {
    parsed = undefined;
    error = err instanceof Error ? err.message : "Invalid JSON";
  }
  return {
    shell: {
      showTeamBootstrapLoading: false,
      showTeamUnavailable: false,
      onBackToSelector: vi.fn(),
      selectedTeam: { id: "team-1", name: "Team One" } as never,
      isAgentWorkspace: false,
      teamSectionCardClassName: "panel",
      panelSecondaryButtonClassName: "secondary",
      teamWorkbenchWorkspaceShellClassName: "shell",
      tab: "runs",
      activeWorkspaceLens: "channels",
      developerMode: true,
      busy: null,
      selectedTeamHasConfiguredMembers: true,
      selectedTeamDescription: null,
      teamMemberForgeLabel: "Forge",
      teamMemberCopyExistingLabel: "Copy",
      onOpenTeamMemberForge: vi.fn(),
      onOpenTeamMemberCopyExisting: vi.fn(),
      showRunContextLoading: false,
      showNoActiveRunNotice: false,
      onGoToRuns: vi.fn(),
    },
    header: {
      workspaceEyebrow: null,
      showDedicatedWorkspaceHeading: false,
      workspaceTitle: "Team One",
      workspaceDescription: null,
      selectedAgentLabel: "",
      selectedAgentWorkspaceMemberId: "",
      selectedAgentStatusView: {
        role: "worker",
        lifecycle: "Idle",
        work: "Idle",
        inbox: "0 pending",
        currentWork: "",
      },
      selectedAgentSpecDraft: null,
      selectedAgentControlState: { canStart: false, canStop: false, canDelete: false },
      showWorkspaceRuntimeBadge: false,
      selectedTeamRuntimeStatus: null,
      selectedTeamRuntimeControlTone: { statusColor: "gray", countColor: "gray" },
      workspaceAdvancedTabItems: [],
      isAdvancedWorkspace: false,
      showRunActionsInAdvanced: false,
      canResumeActiveRun: false,
      canRestartActiveRun: false,
      workspaceDetailsOpen: false,
      workspaceDetailItems: [],
      workspaceNoticeText: null,
      workspaceNoticeDotClassName: "",
      teamWorkbenchMutedButtonClassName: "",
      teamWorkbenchHeaderActionButtonClassName: "",
      workspaceToolbarClassName: "",
      workspaceToolbarButtonActiveClassName: "",
      workspaceToolbarButtonIdleClassName: "",
      workspaceNoticeClassName: "",
      workspaceNoticeTextClassName: "",
      teamRunMetaItemClassName: "",
      onTabChange: vi.fn(),
      onToggleWorkspaceDetails: vi.fn(),
      onRefreshActiveRun: vi.fn(),
      onCancelRun: vi.fn(),
      onResumeRun: vi.fn(),
      onRestartRun: vi.fn(),
      onOpenTeamMemberEditModal: vi.fn(),
      onStartSelectedTeamAgent: vi.fn(),
      onStopSelectedTeamAgent: vi.fn(),
      onDeleteSelectedTeamAgent: vi.fn(),
    },
    runs: {
      onDeleteTeam: vi.fn(),
      runStatusFilter: "all",
      TEAM_RUN_STATUS_FILTER_OPTIONS: [],
      onRunStatusFilterChange: vi.fn(),
      onRefreshRuns: vi.fn(),
      runsLoading: false,
      visibleRuns: [activeRun],
      activeRunIdForSelectedTeam: "run-1",
      setActiveRunId: vi.fn(),
      isActiveRunHiddenByFilter: false,
      activeRunForSelectedTeam: activeRun,
      totalLoadedRunsForTeam: 1,
      runsHasMore: false,
      effectiveSelectedTeamId: "team-1",
      onLoadMoreRuns: vi.fn(),
    },
    overview: {
      snapshot: null,
      snapshotLoading: false,
      onRefreshOverviewSnapshot: vi.fn(),
      mailboxDisplayNameByActorId: {},
    },
    memberConsole: {
      selectedAgentWorkspaceSessionId: null,
      memberEvents: [],
      memberEventsLoading: false,
      memberEventsHasMore: false,
      onLoadOlderMemberConsole: vi.fn(),
      onRefreshMemberConsole: vi.fn(),
    },
    debugRun: {
      teamDebugTag: "run_ops",
      setTeamDebugTag: vi.fn(),
      runContextId: "ctx-1",
      setRunContextId: vi.fn(),
      runInput,
      setRunInput,
      runLookupId: "",
      setRunLookupId: vi.fn(),
      canCreateRun: true,
      runInputHasError: Boolean(error),
      runInputValidation: { parsed, error },
      teamExecutionBlockedReason: null,
      onCreateRun: vi.fn(),
      onLoadRunById: vi.fn(),
      steps: [],
      onRefreshActiveRunSteps: vi.fn(),
      stepKey: "",
      setStepKey: vi.fn(),
      stepMemberId: "",
      onStepMemberIdChange: vi.fn(),
      stepDependsOn: "",
      onStepDependsOnChange: vi.fn(),
      stepInput: "",
      onStepInputChange: vi.fn(),
      onSubmitStep: vi.fn(),
      selectedStepId: "",
      setSelectedStepId: vi.fn(),
      stepAction: "complete",
      setStepAction: vi.fn(),
      stepRemoteTaskId: "",
      onStepRemoteTaskIdChange: vi.fn(),
      stepOutput: "",
      onStepOutputChange: vi.fn(),
      stepFailText: "",
      onStepFailTextChange: vi.fn(),
      stepInputReason: "",
      onStepInputReasonChange: vi.fn(),
      stepInputRequiredPayload: "",
      onStepInputRequiredPayloadChange: vi.fn(),
      stepResumePayload: "",
      onStepResumePayloadChange: vi.fn(),
      onApplyStepAction: vi.fn(),
    },
    conversation: {
      unreadByMemberId: {},
      chatActors: { fromActorId: "", toActorId: "", inboxActorId: "" },
      chatStickToBottom: true,
      chatMessagesRef: { current: null },
      onConversationScroll: vi.fn(),
      onJumpConversationToBottom: vi.fn(),
      conversationMessages: [],
      onAcceptMessage: vi.fn(),
      onAcceptVisibleMessages: vi.fn(),
      onSendChatMessage: vi.fn(),
      MAILBOX_TEMPLATE_OPTIONS: [],
      onMailboxTemplateChange: vi.fn(),
      onApplyMessageTemplate: vi.fn(),
      onSendMessage: vi.fn(),
      onRefreshInbox: vi.fn(),
      chatDraft: "",
      onChatDraftChange: vi.fn(),
    },
    memberAcp: {
      selectedAgentWorkspaceSnapshot: null,
      selectedMemberSnapshot: null,
      selectedAgentWorkspaceRuntimeMember: null,
      selectedAgentWorkspaceAgent: null,
      oldestMemberEventId: null,
      onSendAgentAcpInput: vi.fn(),
      onCancelTeamMemberAcp: vi.fn(),
      onSetTeamMemberAcpMode: vi.fn(),
      onSetTeamMemberAcpModel: vi.fn(),
      onSetTeamMemberAcpConfig: vi.fn(),
      onForceNewTeamMemberSession: vi.fn(),
      memberTargetNodeById: {},
      selectedMemberId: "",
      setSelectedMemberId: vi.fn(),
      selectedMemberDiscoveryCard: null,
      selectedMemberDiscoveryCardLoading: false,
      onOpenMailboxForMember: vi.fn(),
    },
    events: {
      eventsLoading: false,
      oldestEventId: null,
      displayedRunEvents: [],
      previewMode: false,
      eventsAutoRefresh: false,
      setEventsAutoRefresh: vi.fn(),
      onRefreshEventsPanel: vi.fn(),
      onLoadOlderEventsPanel: vi.fn(),
      eventsHasMore: false,
      TEAM_EVENT_PREVIEW_LIMIT: 5,
    },
    mailboxDebug: {
      msgFromActorId: "",
      onMsgFromActorIdChange: vi.fn(),
      msgToActorId: "",
      onMsgToActorIdChange: vi.fn(),
      msgChannel: "",
      onMsgChannelChange: vi.fn(),
      msgTransport: "local",
      onMsgTransportChange: vi.fn(),
      msgRoute: "",
      onMsgRouteChange: vi.fn(),
      msgTemplate: "",
      msgPayload: "",
      onMsgPayloadChange: vi.fn(),
      msgIdempotencyKey: "",
      onMsgIdempotencyKeyChange: vi.fn(),
      inboxActorId: "",
      onInboxActorIdChange: vi.fn(),
      inboxLimit: "",
      onInboxLimitChange: vi.fn(),
      inboxAfterId: "",
      onInboxAfterIdChange: vi.fn(),
      inboxIncludeDelivered: false,
      onInboxIncludeDeliveredChange: vi.fn(),
      mailboxHasActiveRun: true,
      mailboxEmptyTitle: "Mailbox",
      mailboxEmptyBody: "No messages.",
    },
  };
}

function buildWorkspaceContext(workbench: TeamWorkbenchRuntimeContext): TeamWorkspaceContextValue {
  return {
    workbench,
    developerMode: true,
    busy: null,
    snapshot: null,
    mailboxDisplayNameByActorId: {},
    selectedTeamMemberLiveStates: [],
    selectedConversationMatchesChannelLane: true,
    routeThreadRootMessageId: null,
    routeSelectedMemberId: "",
    effectiveSelectedTeamId: "team-1",
    routeWorkspaceLens: "channels",
    routeChannelId: "all",
    activeChannelConversationTaskId: null,
    navigateTeamRoute: vi.fn(),
    isCompactWorkbench: false,
    selectedChannelItem: null,
    selectedConversation: null,
    token: "token",
    taskMessageDraft: "",
    setTaskMessageDraft: vi.fn(),
    onSendTaskMessage: vi.fn(),
    taskMessages: [],
    conversationMailboxMessages: [],
    taskConversationMemberIds: [],
    activeConversationTitle: "# all",
    taskMessagesLoading: false,
    channelFocusMessageId: null,
    setChannelFocusMessageId: vi.fn(),
    onSendThreadReply: vi.fn(),
    threadReplyDraft: "",
    setThreadReplyDraft: vi.fn(),
    tasksLoading: false,
    onRefreshTasks: vi.fn(),
    workspaceTasks: [],
    selectedTaskId: "",
    selectedTaskDetail: null,
    setSelectedTaskId: vi.fn(),
    onSelectConversationSubject: vi.fn(),
    runs: [],
    onOpenTaskRun: vi.fn(),
    compilePreviewContextId: "",
    setCompilePreviewContextId: vi.fn(),
    onCompileTaskRunPreview: vi.fn(),
    canCompileTask: false,
    compiledRunPreview: null,
    onUseCompiledRunPayload: vi.fn(),
    onCreateRunFromCompiledPreview: vi.fn(),
  };
}

function Harness() {
  const [runInput, setRunInput] = React.useState("");
  const workbench = React.useMemo(
    () => buildRuntimeContext(runInput, setRunInput),
    [runInput]
  );
  return (
    <TeamWorkspaceProvider value={buildWorkspaceContext(workbench)}>
      <TeamWorkbenchContainer />
    </TeamWorkspaceProvider>
  );
}

describe("TeamWorkbenchContainer", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("routes run input helper actions through the debug run panel", () => {
    act(() => {
      root.render(<Harness />);
    });

    act(() => {
      findButton(container, "Format").click();
    });
    expect(container.querySelector("[data-testid='run-input']")?.textContent).toBe("{}");

    act(() => {
      findButton(container, "Example").click();
    });
    expect(container.querySelector("[data-testid='run-input']")?.textContent).toContain(
      "improve-team-run"
    );

    act(() => {
      findButton(container, "Format").click();
    });
    expect(container.querySelector("[data-testid='run-input']")?.textContent).toBe(
      '{\n  "task": "investigate",\n  "objective": "improve-team-run"\n}'
    );

    act(() => {
      findButton(container, "Clear").click();
    });
    expect(container.querySelector("[data-testid='run-input']")?.textContent).toBe("");

    act(() => {
      findButton(container, "Empty object").click();
    });
    expect(container.querySelector("[data-testid='run-input']")?.textContent).toBe("{}");
  });
});
