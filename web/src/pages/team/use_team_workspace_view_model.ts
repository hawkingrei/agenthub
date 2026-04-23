import { useCallback, useMemo, type Dispatch, type SetStateAction } from "react";
import type {
  AgentRecord,
  TeamDefinitionRecord,
  TeamPromptDefaultsRecord,
  TeamRunRecord,
  TeamTaskRecord,
} from "../../api";
import type { WorkspaceLens } from "../../app_route_selection";
import { createDisplayNameLookup } from "./mailbox_helpers";
import { describeTeamKanban } from "./channel_metadata";
import type { TeamMemberLiveState, TeamMemberAgentStatusSummary } from "./member_helpers";
import { normalizeTeamMemberLifecycle } from "../team_member_status_strip";
import {
  isSharedThreadTask,
  resolveAgentWorkspaceStatusView,
  resolveSelectedAgentWorkspaceLabel,
  type AgentWorkspaceStatusView,
} from "./page_helpers";
import { buildTeamMemberDraftFromSpec, type TeamMemberProfileDraft } from "./create_helpers";
import { TEAM_TAB_ITEMS, tabRequiresActiveRun, type TeamTab } from "./state";

type UseTeamWorkspaceViewModelOptions = {
  selectedTeam: TeamDefinitionRecord | null;
  routeWorkspaceLens: WorkspaceLens | null;
  tab: TeamTab;
  focusedAgentMemberId: string;
  selectedMemberId: string;
  selectedTeamId: string | null;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  selectedTeamMemberSummary: TeamMemberAgentStatusSummary | null;
  selectedTeamRuntimeStatus: {
    label: string;
    status: string;
  };
  selectedAgentWorkspaceMemberId: string;
  selectedAgentWorkspaceLiveState: TeamMemberLiveState | null;
  activeRunForSelectedTeam: TeamRunRecord | null;
  activeRunIdForSelectedTeam: string | null;
  selectedConversation: TeamTaskRecord | null;
  selectedChannelLabel: string;
  selectedChannelDescription: string;
  runsLoading: boolean;
  isCompactWorkbench: boolean;
  teamPromptDefaults: TeamPromptDefaultsRecord;
  teamMemberAgentsById: Record<string, AgentRecord | null>;
  agents: AgentRecord[];
  setTab: (tab: TeamTab) => void;
  setFocusedAgentMemberId: Dispatch<SetStateAction<string>>;
  setSelectedConversationTaskId: Dispatch<SetStateAction<string>>;
  setSelectedMemberId: Dispatch<SetStateAction<string>>;
  setTeamsSidebarCollapsed: Dispatch<SetStateAction<boolean>>;
  setActiveRunId: Dispatch<SetStateAction<string | null>>;
  setRunLookupId: (next: string) => void;
  navigateToTeamLens: (teamId: string, lens: WorkspaceLens) => void;
  navigateToTeamDetail: (teamId: string) => void;
  navigateToTeamMemberWorkspace: (teamId: string, memberId: string, tab: TeamTab) => void;
  navigateToSidebarTeam: (teamId: string) => void;
  prefetchWorkspaceLens?: (lens: WorkspaceLens) => void;
};

export function useTeamWorkspaceViewModel(options: UseTeamWorkspaceViewModelOptions) {
  const {
    selectedTeam,
    routeWorkspaceLens,
    tab,
    focusedAgentMemberId,
    selectedMemberId,
    selectedTeamId,
    selectedTeamMemberLiveStates,
    selectedTeamMemberSummary,
    selectedTeamRuntimeStatus,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceLiveState,
    activeRunForSelectedTeam,
    activeRunIdForSelectedTeam,
    selectedConversation,
    selectedChannelLabel,
    selectedChannelDescription,
    runsLoading,
    isCompactWorkbench,
    teamPromptDefaults,
    teamMemberAgentsById,
    agents,
    setTab,
    setFocusedAgentMemberId,
    setSelectedConversationTaskId,
    setSelectedMemberId,
    setTeamsSidebarCollapsed,
    setActiveRunId,
    setRunLookupId,
    navigateToTeamLens,
    navigateToTeamDetail,
    navigateToTeamMemberWorkspace,
    navigateToSidebarTeam,
    prefetchWorkspaceLens,
  } = options;

  const selectedMemberLiveState = useMemo(
    () => selectedTeamMemberLiveStates.find((member) => member.member_id === selectedMemberId) ?? null,
    [selectedMemberId, selectedTeamMemberLiveStates]
  );
  const selectedAgentLiveState = useMemo(
    () => selectedTeamMemberLiveStates.find((member) => member.member_id === focusedAgentMemberId) ?? null,
    [focusedAgentMemberId, selectedTeamMemberLiveStates]
  );
  const hasSelectedAgentContext = focusedAgentMemberId.trim().length > 0;
  const isAgentWorkspace =
    hasSelectedAgentContext && (tab === "agent_acp" || tab === "mailbox" || tab === "member_console");

  const activeWorkspaceLens = routeWorkspaceLens ?? resolveWorkspaceLensForTab(tab);
  const workspaceLensItems = useMemo(
    () => [
      {
        value: "channels",
        label: "Channels",
        active: activeWorkspaceLens === "channels",
        onPrefetch: () => prefetchWorkspaceLens?.("channels"),
      },
      {
        value: "tasks",
        label: "Tasks",
        active: activeWorkspaceLens === "tasks",
        onPrefetch: () => prefetchWorkspaceLens?.("tasks"),
      },
      {
        value: "members",
        label: "Members",
        active: activeWorkspaceLens === "members",
        onPrefetch: () => prefetchWorkspaceLens?.("members"),
      },
      {
        value: "search",
        label: "Search",
        active: activeWorkspaceLens === "search",
        onPrefetch: () => prefetchWorkspaceLens?.("search"),
      },
    ],
    [activeWorkspaceLens, prefetchWorkspaceLens]
  );

  const selectedAgentFallbackName = useMemo(() => {
    const memberId = focusedAgentMemberId.trim();
    if (!memberId) {
      return null;
    }
    return (
      teamMemberAgentsById[memberId]?.name?.trim() ??
      agents.find((agent) => agent.id === memberId)?.name?.trim() ??
      null
    );
  }, [agents, focusedAgentMemberId, teamMemberAgentsById]);

  const selectedMemberFallbackName = useMemo(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId) {
      return null;
    }
    return (
      teamMemberAgentsById[memberId]?.name?.trim() ??
      agents.find((agent) => agent.id === memberId)?.name?.trim() ??
      null
    );
  }, [agents, selectedMemberId, teamMemberAgentsById]);

  const selectedAgentLabel = useMemo(
    () =>
      resolveSelectedAgentWorkspaceLabel(
        focusedAgentMemberId,
        selectedAgentLiveState,
        selectedAgentFallbackName
      ),
    [focusedAgentMemberId, selectedAgentFallbackName, selectedAgentLiveState]
  );
  const selectedMailboxLabel = useMemo(
    () =>
      resolveSelectedAgentWorkspaceLabel(
        selectedMemberId,
        selectedMemberLiveState,
        selectedMemberFallbackName
      ),
    [selectedMemberFallbackName, selectedMemberId, selectedMemberLiveState]
  );

  const selectedAgentSpecDraft: TeamMemberProfileDraft | null = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return buildTeamMemberDraftFromSpec(
      selectedTeam.spec,
      selectedAgentWorkspaceMemberId,
      selectedAgentWorkspaceMemberId
        ? teamMemberAgentsById[selectedAgentWorkspaceMemberId] ?? null
        : null,
      teamPromptDefaults
    );
  }, [selectedAgentWorkspaceMemberId, selectedTeam, teamMemberAgentsById, teamPromptDefaults]);

  const selectedAgentStatusView: AgentWorkspaceStatusView = useMemo(
    () => resolveAgentWorkspaceStatusView(selectedAgentLiveState),
    [selectedAgentLiveState]
  );

  const activeConversationTitle = useMemo(
    () => selectedConversation?.title?.trim() || "all",
    [selectedConversation]
  );
  const selectedConversationIsShared = useMemo(
    () => (selectedConversation ? isSharedThreadTask(selectedConversation) : true),
    [selectedConversation]
  );

  const currentWorkspaceTabLabel = useMemo(
    () => TEAM_TAB_ITEMS.find((item) => item.value === tab)?.label ?? selectedTeam?.name ?? "Team",
    [selectedTeam?.name, tab]
  );

  const workspaceTitle = !selectedTeam
    ? "Team Workbench"
    : isAgentWorkspace
      ? selectedAgentLabel
      : tab === "conversation"
        ? selectedConversationIsShared
          ? selectedChannelLabel
          : activeConversationTitle
        : tab === "tasks"
          ? "Kanban"
          : tab === "mailbox"
            ? selectedMemberLiveState
              ? selectedMailboxLabel
              : "Execution Mailbox"
            : selectedMemberLiveState && isAgentWorkspace
              ? selectedAgentLabel
              : currentWorkspaceTabLabel;

  const workspaceDescription = !selectedTeam
    ? "Select a team from the left rail to start team conversations and supervise execution."
    : isAgentWorkspace
      ? null
      : tab === "conversation"
        ? selectedConversationIsShared
          ? selectedChannelDescription
          : "Task thread for the selected Team task. Use it for task-scoped follow-up and execution context."
        : tab === "tasks"
          ? describeTeamKanban(selectedChannelLabel)
          : tab === "mailbox"
            ? selectedMemberLiveState
              ? "Direct mailbox thread for the selected member."
              : "Run-scoped mailbox delivery and direct member conversations."
            : tab === "runs"
              ? "Browse runs and choose the active execution context."
              : isAgentWorkspace
                ? "Direct thread for the selected agent."
                : "Operational views stay available without displacing the main thread.";

  const showWorkspaceRuntimeBadge = !isAgentWorkspace && tab !== "conversation" && tab !== "tasks";
  const showDedicatedWorkspaceHeading =
    isAgentWorkspace || !selectedTeam || workspaceTitle !== selectedTeam.name;

  const workspaceMemberAvailability = useMemo(() => {
    if (selectedTeamMemberSummary) {
      return {
        online: selectedTeamMemberSummary.active,
        offline: selectedTeamMemberSummary.inactive,
        missing: selectedTeamMemberSummary.missing,
      };
    }
    let online = 0;
    let offline = 0;
    let missing = 0;
    for (const member of selectedTeamMemberLiveStates) {
      const lifecycle = normalizeTeamMemberLifecycle(member);
      if (lifecycle === "missing") {
        missing += 1;
      } else if (lifecycle === "stopped") {
        offline += 1;
      } else {
        online += 1;
      }
    }
    return { online, offline, missing };
  }, [selectedTeamMemberLiveStates, selectedTeamMemberSummary]);

  const workspaceNoticeText = useMemo(() => {
    if (isAgentWorkspace || tab === "conversation" || tab === "tasks") {
      return null;
    }
    const runtimeLabel = selectedTeam ? selectedTeamRuntimeStatus.label : null;
    const runLabel = activeRunForSelectedTeam ? `run ${activeRunForSelectedTeam.status}` : "no active run";
    const rosterLabel = `${selectedTeamMemberLiveStates.length} members`;
    const availabilityLabel =
      workspaceMemberAvailability.missing > 0
        ? `${workspaceMemberAvailability.missing} missing`
        : workspaceMemberAvailability.offline > 0
          ? `${workspaceMemberAvailability.offline} offline`
          : `${workspaceMemberAvailability.online} online`;
    return [runtimeLabel, runLabel, rosterLabel, availabilityLabel]
      .filter((value): value is string => Boolean(value))
      .join(" · ");
  }, [
    activeRunForSelectedTeam,
    isAgentWorkspace,
    selectedTeam,
    selectedTeamMemberLiveStates.length,
    selectedTeamRuntimeStatus.label,
    tab,
    workspaceMemberAvailability,
  ]);

  const workspaceNoticeDotClassName = useMemo(() => {
    const base = "inline-flex h-2 w-2 shrink-0 rounded-full";
    if (isAgentWorkspace) {
      if (selectedAgentStatusView.lifecycle === "missing" || selectedAgentStatusView.work === "blocked") {
        return `${base} bg-rose-500`;
      }
      if (
        selectedAgentStatusView.lifecycle === "working" ||
        selectedAgentStatusView.work === "working" ||
        selectedAgentStatusView.work === "pending" ||
        selectedAgentStatusView.work === "done"
      ) {
        return `${base} bg-emerald-500`;
      }
      if (selectedAgentStatusView.lifecycle === "stopped") {
        return `${base} bg-slate-400`;
      }
    }
    if (workspaceMemberAvailability.missing > 0) return `${base} bg-rose-500`;
    if (selectedTeamRuntimeStatus.status === "degraded") return `${base} bg-amber-500`;
    if (selectedTeamRuntimeStatus.status === "stopped") return `${base} bg-slate-400`;
    if (!activeRunForSelectedTeam) return `${base} bg-slate-400`;
    if (activeRunForSelectedTeam.status === "working" || activeRunForSelectedTeam.status === "completed") {
      return `${base} bg-emerald-500`;
    }
    if (activeRunForSelectedTeam.status === "failed") return `${base} bg-rose-500`;
    if (activeRunForSelectedTeam.status === "canceled") return `${base} bg-amber-500`;
    return `${base} bg-slate-400`;
  }, [activeRunForSelectedTeam, isAgentWorkspace, selectedAgentStatusView, selectedTeamRuntimeStatus.status, workspaceMemberAvailability.missing]);

  const workspaceDetailItems = useMemo(
    () =>
      [
        `team=${selectedTeam?.id ?? "-"}`,
        `team_runtime=${selectedTeamRuntimeStatus.status}`,
        `active_run=${activeRunIdForSelectedTeam ?? "-"}`,
        `run_status=${activeRunForSelectedTeam?.status ?? "-"}`,
        `context=${activeRunForSelectedTeam?.context_id ?? "-"}`,
        isAgentWorkspace &&
        selectedAgentWorkspaceLiveState?.agent_name &&
        selectedAgentWorkspaceLiveState.agent_name !== selectedAgentWorkspaceLiveState.member_id
          ? `agent=${selectedAgentWorkspaceLiveState.agent_name}`
          : null,
        isAgentWorkspace ? `member=${selectedAgentWorkspaceLiveState?.member_id ?? "-"}` : null,
      ].filter((value): value is string => value !== null),
    [
      activeRunForSelectedTeam?.context_id,
      activeRunForSelectedTeam?.status,
      activeRunIdForSelectedTeam,
      isAgentWorkspace,
      selectedAgentWorkspaceLiveState?.agent_name,
      selectedAgentWorkspaceLiveState?.member_id,
      selectedTeam?.id,
      selectedTeamRuntimeStatus.status,
    ]
  );

  const mailboxDisplayNameByActorId = useMemo(
    () =>
      createDisplayNameLookup([
        ["user", "You"],
        ...selectedTeamMemberLiveStates.map((member) => [
          member.member_id,
          member.agent_name?.trim() || member.member_id,
        ]),
      ]),
    [selectedTeamMemberLiveStates]
  );

  const onOpenTaskRun = useCallback(
    (runId: string) => {
      setActiveRunId(runId);
      setRunLookupId(runId);
      setFocusedAgentMemberId("");
      setTab("runs");
    },
    [setActiveRunId, setFocusedAgentMemberId, setRunLookupId, setTab]
  );

  const onOpenMailboxForMember = useCallback(
    (memberId: string) => {
      setSelectedMemberId(memberId);
      setFocusedAgentMemberId(memberId);
      setTab("mailbox");
      if (selectedTeamId) {
        navigateToTeamMemberWorkspace(selectedTeamId, memberId, "mailbox");
      }
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [
      isCompactWorkbench,
      navigateToTeamMemberWorkspace,
      selectedTeamId,
      setFocusedAgentMemberId,
      setSelectedMemberId,
      setTab,
      setTeamsSidebarCollapsed,
    ]
  );

  const onSelectConversationSubject = useCallback(
    (taskId?: string | null) => {
      setFocusedAgentMemberId("");
      setSelectedConversationTaskId(typeof taskId === "string" ? taskId.trim() : "");
      setTab("conversation");
      if (selectedTeamId) {
        navigateToTeamLens(selectedTeamId, "channels");
      }
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [isCompactWorkbench, navigateToTeamLens, selectedTeamId, setFocusedAgentMemberId, setSelectedConversationTaskId, setTab, setTeamsSidebarCollapsed]
  );

  const onSelectKanbanSubject = useCallback(() => {
    setFocusedAgentMemberId("");
    setTab("tasks");
    if (selectedTeamId) {
      navigateToTeamLens(selectedTeamId, "tasks");
    }
    if (isCompactWorkbench) {
      setTeamsSidebarCollapsed(true);
    }
  }, [isCompactWorkbench, navigateToTeamLens, selectedTeamId, setFocusedAgentMemberId, setTab, setTeamsSidebarCollapsed]);

  const onSelectAgentWorkspace = useCallback(
    (memberId: string, nextTab: TeamTab = "agent_acp") => {
      setSelectedMemberId(memberId);
      setFocusedAgentMemberId(memberId);
      setTab(nextTab);
      if (selectedTeamId) {
        navigateToTeamMemberWorkspace(selectedTeamId, memberId, nextTab);
      }
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [
      isCompactWorkbench,
      navigateToTeamMemberWorkspace,
      selectedTeamId,
      setFocusedAgentMemberId,
      setSelectedMemberId,
      setTab,
      setTeamsSidebarCollapsed,
    ]
  );

  const onSelectUtilityWorkspace = useCallback(
    (nextTab: TeamTab) => {
      setFocusedAgentMemberId("");
      setTab(nextTab);
      if (selectedTeamId) {
        navigateToTeamDetail(selectedTeamId);
      }
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [
      isCompactWorkbench,
      navigateToTeamDetail,
      selectedTeamId,
      setFocusedAgentMemberId,
      setTab,
      setTeamsSidebarCollapsed,
    ]
  );

  const onSelectSidebarTeam = useCallback(
    (teamId: string) => {
      if (teamId !== selectedTeamId) {
        navigateToSidebarTeam(teamId);
      }
    },
    [navigateToSidebarTeam, selectedTeamId]
  );

  const onSelectWorkspaceLens = useCallback(
    (value: string) => {
      if (!selectedTeamId) {
        return;
      }
      const lens = value as WorkspaceLens;
      if (lens === "search") {
        setFocusedAgentMemberId("");
      }
      navigateToTeamLens(selectedTeamId, lens);
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [isCompactWorkbench, navigateToTeamLens, selectedTeamId, setFocusedAgentMemberId, setTeamsSidebarCollapsed]
  );

  const tabNeedsActiveRun = tabRequiresActiveRun(tab);
  const showRunContextLoading =
    tab !== "runs" && tabNeedsActiveRun && runsLoading && activeRunForSelectedTeam == null;
  const showNoActiveRunNotice =
    tab !== "runs" && tabNeedsActiveRun && !runsLoading && activeRunForSelectedTeam == null;

  return {
    activeWorkspaceLens,
    workspaceLensItems,
    selectedMemberLiveState,
    selectedAgentLiveState,
    isAgentWorkspace,
    selectedAgentLabel,
    selectedAgentSpecDraft,
    selectedAgentStatusView,
    selectedMailboxLabel,
    activeConversationTitle,
    selectedConversationIsShared,
    workspaceTitle,
    workspaceDescription,
    showWorkspaceRuntimeBadge,
    showDedicatedWorkspaceHeading,
    workspaceNoticeText,
    workspaceNoticeDotClassName,
    workspaceDetailItems,
    mailboxDisplayNameByActorId,
    onOpenTaskRun,
    onOpenMailboxForMember,
    onSelectConversationSubject,
    onSelectKanbanSubject,
    onSelectAgentWorkspace,
    onSelectUtilityWorkspace,
    onSelectSidebarTeam,
    onSelectWorkspaceLens,
    showRunContextLoading,
    showNoActiveRunNotice,
  };
}

function resolveWorkspaceLensForTab(tab: TeamTab): WorkspaceLens {
  switch (tab) {
    case "tasks":
      return "tasks";
    case "overview":
      return "members";
    default:
      return "channels";
  }
}
