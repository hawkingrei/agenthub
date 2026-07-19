import React from "react";
import { CloseButton, Menu, Modal, TextInput, UnstyledButton } from "@mantine/core";
import { DeterministicAvatar } from "../components/deterministic_avatar";
import { TeamDefinitionRecord, type TeamTaskRecord } from "../api";
import {
  NOTION_FLOATING_MENU_PROPS,
  NOTION_MODAL_CLASSNAMES,
  NOTION_MODAL_OVERLAY_PROPS,
} from "../ui/floating_surfaces";
import { AlphaBadge, IconButton } from "../ui/primitives";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_SIDEBAR_META_GRID_CLASS,
  TEAM_SIDEBAR_ROOT_CLASS,
  TEAM_SIDEBAR_SECTION_CLASS,
  TEAM_SIDEBAR_SECTION_TOGGLE_CLASS,
  TEAM_SIDEBAR_NAV_LIST_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS,
  TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS,
  TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS,
  TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS,
  TEAM_SOFT_CHROME_SHADOW_CLASS,
  TEAM_SIDEBAR_WORK_CLASS,
  TEAM_SIDEBAR_INDICATOR_DOT_CLASS,
} from "../ui/tailwind_classes";
import {
  DEFAULT_TEAM_CHANNEL_ID,
  DEFAULT_TEAM_CHANNEL_ITEMS,
  describeTeamKanban,
  type TeamChannelItem,
} from "./team/channel_metadata";
import { TeamMemberLiveState } from "./team/member_helpers";
import { normalizeTeamMemberLifecycle, normalizeTeamMemberWorkStatus } from "./team_member_status_strip";
import type { TeamTab } from "./team/state";
import {
  resolveTeamSidebarSubjectPane,
  type TeamSidebarSubjectPane,
  type WorkspaceLens,
} from "./team/team_route_helpers";

type TeamMemberSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

type TeamSidebarProps = {
  showTeamSelector?: boolean;
  isRoot?: boolean;
  developerMode: boolean;
  busy: string | null;
  onRefreshTeams: () => Promise<void> | void;
  onOpenCreateTeam: () => void;
  draftTeamName: string;
  coordinatorMemberId: string;
  configuredWorkerCount: number;
  teams: TeamDefinitionRecord[];
  selectedTeam: TeamDefinitionRecord | null;
  selectedTeamId: string | null;
  selectedTeamRuntimeStatus?: {
    label: string;
    online: number;
    total: number;
    status: string;
  };
  selectedTeamHasConfiguredMembers?: boolean;
  teamMemberSummaryByTeamId: Map<string, TeamMemberSummary>;
  memberLiveStates: TeamMemberLiveState[];
  memberTargetNodeById?: Record<string, string | null>;
  channelItems?: ReadonlyArray<TeamChannelItem>;
  workspaceTasks?: ReadonlyArray<TeamTaskRecord>;
  selectedChannelId?: TeamChannelItem["id"];
  focusedAgentMemberId: string;
  activeWorkspaceLens?: WorkspaceLens;
  tab: TeamTab;
  onSelectTeam: (teamId: string) => void;
  onBackToSelector?: () => void;
  onSelectChannel: (channelId: TeamChannelItem["id"]) => void;
  onCreateChannel?: (payload: {
    channelId: string;
    description: string;
  }) => Promise<void> | void;
  onDeleteChannel?: (channelId: TeamChannelItem["id"]) => Promise<void> | void;
  creatingChannel?: boolean;
  deletingChannelId?: string | null;
  onSelectKanban: () => void;
  onSelectTask?: (taskId: string) => void;
  onSelectSearch?: () => void;
  onSelectAgentTab: (memberId: string, tab: TeamTab) => void;
  onOpenTeamMemberForge?: () => void;
  onOpenTeamMemberCopyExisting?: () => void;
  teamMemberForgeLabel?: string;
  teamMemberCopyExistingLabel?: string;
  onStartTeamRuntime?: () => void;
  onStopTeamRuntime?: () => void;
  onOpenMachines?: () => void;
  currentMachineId?: string | null;
  onOpenCurrentMachine?: (() => void) | null;
};

const AGENT_FOCUS_TABS = new Set<TeamTab>(["agent_acp", "member_console", "mailbox"]);
type TeamSidebarSection = "teams" | "agents";
const NO_ACTIVE_RUN_CONTEXT = "No active run context.";
const DEBUG_CURRENT_WORK_PATTERN = /^(?:run_status|step_status)\s*=/i;
type TeamSidebarSearchResult = {
  key: string;
  label: string;
  description: string;
  icon: string;
  section: "Channels" | "Tasks" | "Agents";
  onSelect: () => void;
};

function humanizeToken(value: string): string {
  return value
    .split(/[_-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function resolveMemberPrimaryLabel(member: TeamMemberLiveState): string {
  const agentName = member.agent_name?.trim();
  if (agentName) {
    return agentName;
  }
  return member.member_id;
}

export function formatWorkLabel(workLabel: string): string {
  if (workLabel === "no_run") {
    return "idle";
  }
  return humanizeToken(workLabel).toLowerCase();
}

function formatLifecycleLabel(
  lifecycle: ReturnType<typeof normalizeTeamMemberLifecycle>
): string {
  if (lifecycle === "working") {
    return "Online";
  }
  if (lifecycle === "stopped") {
    return "Offline";
  }
  return humanizeToken(lifecycle);
}

function formatMemberStateLabel(
  lifecycle: ReturnType<typeof normalizeTeamMemberLifecycle>,
  workStatus: ReturnType<typeof normalizeTeamMemberWorkStatus>
): string {
  if (lifecycle === "missing") {
    return "Missing";
  }
  if (lifecycle === "stopped") {
    return "Stopped";
  }
  if (workStatus === "blocked") {
    return "Blocked";
  }
  if (workStatus === "pending") {
    return "Waiting";
  }
  if (workStatus === "working") {
    return "Working";
  }
  if (workStatus === "done") {
    return "Done";
  }
  if (workStatus === "idle" || workStatus === "no_run") {
    return "Idle";
  }
  return formatLifecycleLabel(lifecycle);
}

function shouldShowMemberStateLabel(label: string): boolean {
  return label.trim().length > 0;
}

function resolveCurrentWorkLabel(member: TeamMemberLiveState): string | null {
  const currentWork = member.current_work?.trim();
  if (!currentWork || currentWork === NO_ACTIVE_RUN_CONTEXT) {
    return null;
  }
  if (DEBUG_CURRENT_WORK_PATTERN.test(currentWork)) {
    return null;
  }
  return currentWork;
}

const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS = "px-2 py-1.5";
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS = TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS;
const TEAM_SIDEBAR_VIRTUAL_LIST_CLASS =
  "[content-visibility:auto] [contain-intrinsic-size:1px_320px]";
const TEAM_SWITCH_BUTTON_CLASS =
  "inline-flex min-w-0 max-w-full items-center gap-2 rounded-xl border border-notion-border bg-white px-2.5 py-1.5 text-left shadow-sm transition hover:border-notion-accent/25 hover:bg-notion-hover";
const TEAM_CONTROLS_BUTTON_CLASS =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-notion-border bg-notion-sidebar/40 text-[12px] text-notion-text-muted shadow-sm transition hover:border-notion-accent/25 hover:bg-notion-hover hover:text-notion-text";
const TEAM_SUBJECT_SWITCHER_SECTION_CLASS =
  "relative z-[1] flex flex-col pb-2 pl-2 pr-3 pt-2";
const TEAM_SUBJECT_SWITCHER_CLASS =
  "flex items-center gap-1";
const TEAM_SUBJECT_SWITCHER_ACTIVE_CLASS =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-notion-hover text-[13px] text-notion-text";
const TEAM_SUBJECT_SWITCHER_IDLE_CLASS =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[13px] text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export function buildTeamSidebarSearchResults(options: {
  query: string;
  channelItems: ReadonlyArray<TeamChannelItem>;
  selectedChannelLabel: string;
  workspaceTasks: ReadonlyArray<TeamTaskRecord>;
  memberLiveStates: ReadonlyArray<TeamMemberLiveState>;
  onSelectChannel: (channelId: TeamChannelItem["id"]) => void;
  onSelectKanban: () => void;
  onSelectTask: (taskId: string) => void;
  onSelectAgent: (memberId: string) => void;
}): TeamSidebarSearchResult[] {
  const {
    query,
    channelItems,
    selectedChannelLabel,
    workspaceTasks,
    memberLiveStates,
    onSelectChannel,
    onSelectKanban,
    onSelectTask,
    onSelectAgent,
  } = options;
  const normalizedQuery = query.trim().toLowerCase();
  const matches = (value: string) =>
    normalizedQuery.length === 0 || value.toLowerCase().includes(normalizedQuery);
  const channels = channelItems
    .filter((channel) => matches(`${channel.label} ${channel.description ?? ""}`))
    .map((channel) => ({
      key: `channel:${channel.id}`,
      label: channel.label,
      description: channel.description || "Channel",
      icon: "bi-chat-left-text",
      section: "Channels" as const,
      onSelect: () => onSelectChannel(channel.id),
    }));
  const kanban = matches(`Kanban ${describeTeamKanban(selectedChannelLabel)}`)
    ? [
        {
          key: "tasks:kanban",
          label: "Kanban",
          description: describeTeamKanban(selectedChannelLabel),
          icon: "bi-kanban",
          section: "Tasks" as const,
          onSelect: onSelectKanban,
        },
      ]
    : [];
  const tasks = workspaceTasks
    .filter((task) =>
      matches(`${task.title} ${task.id} ${task.status} ${task.assigned_member_id ?? ""}`)
    )
    .map((task) => ({
      key: `task:${task.id}`,
      label: task.title || task.id,
      description: task.assigned_member_id
        ? `${task.status} · ${task.assigned_member_id}`
        : task.status,
      icon: "bi-check2-square",
      section: "Tasks" as const,
      onSelect: () => onSelectTask(task.id),
    }));
  const agents = memberLiveStates
    .filter((member) =>
      matches(`${resolveMemberPrimaryLabel(member)} ${resolveCurrentWorkLabel(member) ?? ""}`)
    )
    .map((member) => ({
      key: `agent:${member.member_id}`,
      label: resolveMemberPrimaryLabel(member),
      description: resolveCurrentWorkLabel(member) ?? formatMemberStateLabel(
        normalizeTeamMemberLifecycle(member),
        normalizeTeamMemberWorkStatus(member)
      ),
      icon: "bi-people",
      section: "Agents" as const,
      onSelect: () => onSelectAgent(member.member_id),
    }));
  return [...channels, ...kanban, ...tasks, ...agents].slice(0, 12);
}

export function resolveMemberIndicatorClassName(
  lifecycle: ReturnType<typeof normalizeTeamMemberLifecycle>,
  workStatus: ReturnType<typeof normalizeTeamMemberWorkStatus>
): string {
  if (lifecycle === "missing" || workStatus === "blocked") {
    return "bg-rose-500";
  }
  if (lifecycle === "working" || workStatus === "working" || workStatus === "pending") {
    return "bg-emerald-500";
  }
  if (workStatus === "done") {
    return "bg-emerald-400";
  }
  if (lifecycle === "stopped") {
    return "bg-slate-400";
  }
  return "bg-slate-300";
}

function TeamSidebarImpl(props: TeamSidebarProps) {
  const {
    showTeamSelector = true,
    isRoot = false,
    developerMode,
    busy,
    onRefreshTeams,
    onOpenCreateTeam,
    draftTeamName,
    coordinatorMemberId,
    configuredWorkerCount,
    teams,
    selectedTeam,
    selectedTeamId,
    selectedTeamRuntimeStatus,
    selectedTeamHasConfiguredMembers = false,
    memberLiveStates,
    channelItems = DEFAULT_TEAM_CHANNEL_ITEMS,
    workspaceTasks = [],
    selectedChannelId = "all",
    focusedAgentMemberId,
    activeWorkspaceLens,
    tab,
    onSelectTeam,
    onBackToSelector,
    onSelectChannel,
    onCreateChannel,
    onDeleteChannel,
    creatingChannel = false,
    deletingChannelId = null,
    onSelectKanban,
    onSelectTask = () => {},
    onSelectAgentTab,
    onOpenTeamMemberForge,
    onOpenTeamMemberCopyExisting,
    teamMemberForgeLabel = "Add Agent",
    teamMemberCopyExistingLabel = "Copy Existing Agent",
    onStartTeamRuntime,
    onStopTeamRuntime,
    onOpenMachines,
    currentMachineId = null,
    onOpenCurrentMachine = null,
  } = props;
  const [teamFilter, setTeamFilter] = React.useState("");
  const [teamDetailsOpen, setTeamDetailsOpen] = React.useState(false);
  const [showCreateChannelForm, setShowCreateChannelForm] = React.useState(false);
  const [newChannelId, setNewChannelId] = React.useState("");
  const [newChannelDescription, setNewChannelDescription] = React.useState("");
  const [searchDialogOpen, setSearchDialogOpen] = React.useState(false);
  const [sidebarSearchQuery, setSidebarSearchQuery] = React.useState("");
  const [sectionOpen, setSectionOpen] = React.useState<Record<TeamSidebarSection, boolean>>({
    teams: true,
    agents: true,
  });
  const [activeSubjectPane, setActiveSubjectPane] = React.useState<TeamSidebarSubjectPane>(() =>
    resolveTeamSidebarSubjectPane({ tab, activeWorkspaceLens })
  );
  React.useEffect(() => {
    setActiveSubjectPane(resolveTeamSidebarSubjectPane({ tab, activeWorkspaceLens }));
  }, [activeWorkspaceLens, tab]);
  React.useEffect(() => {
    setShowCreateChannelForm(false);
    setNewChannelId("");
    setNewChannelDescription("");
    setSearchDialogOpen(false);
    setSidebarSearchQuery("");
  }, [selectedTeamId]);
  const deferredTeamFilter = React.useDeferredValue(teamFilter);
  const normalizedTeamFilter = deferredTeamFilter.trim().toLowerCase();
  const filteredTeams = React.useMemo(() => {
    if (!normalizedTeamFilter) {
      return teams;
    }
    return teams.filter((team) => {
      const name = team.name.toLowerCase();
      const id = team.id.toLowerCase();
      return name.includes(normalizedTeamFilter) || id.includes(normalizedTeamFilter);
    });
  }, [normalizedTeamFilter, teams]);
  const hasTeamFilter = normalizedTeamFilter.length > 0;
  const switchableTeams = React.useMemo(
    () => teams.filter((team) => team.id !== selectedTeamId),
    [selectedTeamId, teams]
  );
  const selectedChannelLabel = React.useMemo(
    () =>
      channelItems.find((channel) => channel.id === selectedChannelId)?.label ??
      DEFAULT_TEAM_CHANNEL_ITEMS[0]?.label ??
      "# all",
    [channelItems, selectedChannelId]
  );
  const selectedSidebarChannelId = React.useMemo(
    () =>
      channelItems.find((channel) => channel.id === selectedChannelId)?.id ??
      channelItems[0]?.id ??
      DEFAULT_TEAM_CHANNEL_ID,
    [channelItems, selectedChannelId]
  );
  const defaultSidebarAgentMemberId = React.useMemo(
    () =>
      memberLiveStates.find((member) => member.member_id === focusedAgentMemberId)?.member_id ??
      memberLiveStates[0]?.member_id ??
      "",
    [focusedAgentMemberId, memberLiveStates]
  );
  const closeSearchDialog = React.useCallback(() => {
    setSearchDialogOpen(false);
    setSidebarSearchQuery("");
  }, []);
  const sidebarSearchResults = React.useMemo(() => {
    return buildTeamSidebarSearchResults({
      query: sidebarSearchQuery,
      channelItems,
      selectedChannelLabel,
      workspaceTasks,
      memberLiveStates,
      onSelectChannel: (channelId) => {
        setActiveSubjectPane("channels");
        closeSearchDialog();
        onSelectChannel(channelId);
      },
      onSelectKanban: () => {
        setActiveSubjectPane("tasks");
        closeSearchDialog();
        onSelectKanban();
      },
      onSelectTask: (taskId) => {
        closeSearchDialog();
        onSelectTask(taskId);
      },
      onSelectAgent: (memberId) => {
        setActiveSubjectPane("agents");
        closeSearchDialog();
        onSelectAgentTab(memberId, "agent_acp");
      },
    });
  }, [
    channelItems,
    memberLiveStates,
    onSelectTask,
    onSelectAgentTab,
    onSelectChannel,
    onSelectKanban,
    closeSearchDialog,
    sidebarSearchQuery,
    selectedChannelLabel,
    workspaceTasks,
  ]);

  const toggleSection = React.useCallback((section: TeamSidebarSection) => {
    setSectionOpen((current) => ({
      ...current,
      [section]: !current[section],
    }));
  }, []);
  const canSubmitChannelCreate = newChannelId.trim().length > 0 && !creatingChannel;
  const subjectPaneItems: ReadonlyArray<{
    value: TeamSidebarSubjectPane;
    label: string;
    icon: string;
    ariaLabel: string;
  }> = [
    {
      value: "channels",
      label: "Channels",
      icon: "bi-chat-left-text",
      ariaLabel: "Show channels",
    },
    {
      value: "tasks",
      label: "Tasks",
      icon: "bi-kanban",
      ariaLabel: "Show tasks",
    },
    {
      value: "agents",
      label: "Agents",
      icon: "bi-people",
      ariaLabel: "Show agents",
    },
  ];
  const resetCreateChannelForm = React.useCallback(() => {
    setShowCreateChannelForm(false);
    setNewChannelId("");
    setNewChannelDescription("");
  }, []);
  const handleSelectSubjectPane = React.useCallback(
    (value: TeamSidebarSubjectPane) => {
      setActiveSubjectPane(value);
      if (value === "channels") {
        onSelectChannel(selectedSidebarChannelId);
        return;
      }
      if (value === "tasks") {
        onSelectKanban();
        return;
      }
      if (value === "agents") {
        if (defaultSidebarAgentMemberId) {
          onSelectAgentTab(defaultSidebarAgentMemberId, "agent_acp");
        }
      }
    },
    [
      defaultSidebarAgentMemberId,
      onSelectAgentTab,
      onSelectChannel,
      onSelectKanban,
      selectedSidebarChannelId,
    ]
  );
  const handleOpenSearch = React.useCallback(() => {
    setSearchDialogOpen(true);
  }, []);
  const handleCreateChannelSubmit = React.useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!onCreateChannel || !canSubmitChannelCreate) {
        return;
      }
      const payload = {
        channelId: newChannelId.trim(),
        description: newChannelDescription.trim(),
      };
      try {
        await Promise.resolve(onCreateChannel(payload));
        resetCreateChannelForm();
      } catch {
        // Keep the inline form open so the user can correct the input after a failed create.
      }
    },
    [
      canSubmitChannelCreate,
      newChannelDescription,
      newChannelId,
      onCreateChannel,
      resetCreateChannelForm,
    ]
  );

  return (
    <aside
      className={TEAM_SIDEBAR_ROOT_CLASS}
      data-team-surface="sidebar"
    >
      <div className={TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS}>
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            {showTeamSelector ? (
              <div className="truncate text-[15px] font-semibold tracking-tight text-notion-text">
                {selectedTeam?.name ?? "Select a team"}
              </div>
            ) : selectedTeam ? (
              <div className="flex min-w-0 items-center gap-2">
                <Menu
                  position="bottom-start"
                  {...NOTION_FLOATING_MENU_PROPS}
                >
                  <Menu.Target>
                    <UnstyledButton
                      className={TEAM_SWITCH_BUTTON_CLASS}
                      aria-label={`Switch teams from ${selectedTeam.name}`}
                      title="Switch teams"
                    >
                      <span className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-notion-sidebar/70 text-[11px] text-notion-text-muted">
                        <i className="bi bi-collection" aria-hidden="true" />
                      </span>
                      <span className="truncate text-[15px] font-semibold tracking-tight text-notion-text">
                        {selectedTeam.name}
                      </span>
                      <span className="inline-flex h-4 w-4 shrink-0 items-center justify-center text-[11px] text-notion-text-muted">
                        <i className="bi bi-chevron-expand" aria-hidden="true" />
                      </span>
                    </UnstyledButton>
                  </Menu.Target>
                  <Menu.Dropdown>
                    <Menu.Label>Teams</Menu.Label>
                    {onBackToSelector ? (
                      <>
                        <Menu.Item
                          leftSection={<i className="bi bi-grid-3x3-gap" aria-hidden="true" />}
                          onClick={onBackToSelector}
                        >
                          All Teams
                        </Menu.Item>
                        <Menu.Divider />
                      </>
                    ) : null}
                    {teams.map((team) => (
                      <Menu.Item
                        key={team.id}
                        onClick={() => onSelectTeam(team.id)}
                      >
                        {team.name}
                      </Menu.Item>
                    ))}
                  </Menu.Dropdown>
                </Menu>
                <Menu
                  position="bottom-start"
                  {...NOTION_FLOATING_MENU_PROPS}
                >
                  <Menu.Target>
                    <UnstyledButton
                      className={TEAM_CONTROLS_BUTTON_CLASS}
                      aria-label={`Open controls for ${selectedTeam.name}`}
                      title={`Open controls for ${selectedTeam.name}`}
                    >
                      <i className="bi bi-sliders2" aria-hidden="true" />
                    </UnstyledButton>
                  </Menu.Target>
                  <Menu.Dropdown>
                    <Menu.Label>{selectedTeam.name}</Menu.Label>
                    {switchableTeams.length > 0 && (
                      <>
                        <Menu.Label>Switch team</Menu.Label>
                        {switchableTeams.map((team) => (
                          <Menu.Item
                            key={team.id}
                            onClick={() => onSelectTeam(team.id)}
                          >
                            {team.name}
                          </Menu.Item>
                        ))}
                        <Menu.Divider />
                      </>
                    )}
                    {(onOpenTeamMemberForge ||
                      onOpenTeamMemberCopyExisting ||
                      onStartTeamRuntime ||
                      onStopTeamRuntime) && (
                      <>
                        {isRoot && onOpenMachines && (
                          <Menu.Item
                            leftSection={<i className="bi bi-pc-display" aria-hidden="true" />}
                            onClick={onOpenMachines}
                          >
                            Machines
                          </Menu.Item>
                        )}
                        {isRoot && currentMachineId && onOpenCurrentMachine && (
                          <Menu.Item
                            leftSection={<i className="bi bi-box-arrow-up-right" aria-hidden="true" />}
                            onClick={onOpenCurrentMachine}
                          >
                            {`Current Machine (${currentMachineId})`}
                          </Menu.Item>
                        )}
                        {onOpenTeamMemberForge && (
                          <Menu.Item
                            leftSection={<i className="bi bi-person-plus" aria-hidden="true" />}
                            onClick={onOpenTeamMemberForge}
                          >
                            {teamMemberForgeLabel}
                          </Menu.Item>
                        )}
                        {onOpenTeamMemberCopyExisting && (
                          <Menu.Item
                            leftSection={<i className="bi bi-copy" aria-hidden="true" />}
                            onClick={onOpenTeamMemberCopyExisting}
                          >
                            <div className="flex items-center gap-2">
                              <span>{teamMemberCopyExistingLabel}</span>
                              <AlphaBadge className="px-1.5 py-0 text-[9px]" />
                            </div>
                          </Menu.Item>
                        )}
                        {onStartTeamRuntime && selectedTeamRuntimeStatus && (
                          <Menu.Item
                            leftSection={<i className="bi bi-play-circle" aria-hidden="true" />}
                            onClick={onStartTeamRuntime}
                            disabled={
                              busy === "stop-team" ||
                              selectedTeamRuntimeStatus.status === "running" ||
                              !selectedTeamHasConfiguredMembers
                            }
                          >
                            Start Team
                          </Menu.Item>
                        )}
                        {onStopTeamRuntime && selectedTeamRuntimeStatus && (
                          <Menu.Item
                            leftSection={<i className="bi bi-stop-circle" aria-hidden="true" />}
                            onClick={onStopTeamRuntime}
                            disabled={
                              busy === "start-team" ||
                              selectedTeamRuntimeStatus.status === "stopped"
                            }
                          >
                            Stop Team
                          </Menu.Item>
                        )}
                      </>
                    )}
                    {developerMode && (
                      <>
                        {(onOpenTeamMemberForge ||
                          onStartTeamRuntime ||
                          onStopTeamRuntime ||
                          (isRoot && (onOpenMachines || (currentMachineId && onOpenCurrentMachine)))) && (
                          <Menu.Divider />
                        )}
                        <Menu.Item disabled>
                          <div className="min-w-[220px] text-[12px] leading-5 text-notion-text-muted">
                            <span className="font-semibold text-notion-text">Team ID</span>
                            <span className="ml-2 break-all">{selectedTeam.id}</span>
                          </div>
                        </Menu.Item>
                      </>
                    )}
                  </Menu.Dropdown>
                </Menu>
              </div>
            ) : (
              <div className="truncate text-[15px] font-semibold tracking-tight text-notion-text">
                Teams
              </div>
            )}
          </div>
          {showTeamSelector && (
            <div className="flex items-center gap-1.5 pt-0.5">
              <IconButton
                onClick={() => {
                  void onRefreshTeams();
                }}
                disabled={busy === "refresh-teams"}
                tone="subtle"
                size="md"
                className="h-8 w-8 text-notion-text-muted hover:bg-notion-hover hover:text-notion-text"
                title="Refresh teams"
                aria-label="Refresh teams"
              >
                <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              </IconButton>
              <Menu
                position="bottom-end"
                {...NOTION_FLOATING_MENU_PROPS}
              >
                <Menu.Target>
                  <IconButton
                    tone="default"
                    size="md"
                    className={`${TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS} h-8 w-auto px-2 text-[11px] font-medium text-black/60`}
                    aria-label="Open team actions"
                    title="Open team actions"
                  >
                    <span>More</span>
                  </IconButton>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Item
                    leftSection={<i className="bi bi-plus-lg" aria-hidden="true" />}
                    onClick={onOpenCreateTeam}
                  >
                    Create Team
                  </Menu.Item>
                  {developerMode && (
                    <>
                      <Menu.Divider />
                      <Menu.Item
                        leftSection={<i className="bi bi-info-circle" aria-hidden="true" />}
                        onClick={() => {
                          setTeamDetailsOpen((current) => !current);
                        }}
                      >
                        {teamDetailsOpen ? "Hide Team Details" : "Show Team Details"}
                      </Menu.Item>
                    </>
                  )}
                </Menu.Dropdown>
              </Menu>
            </div>
          )}
        </div>
        {showTeamSelector && teamDetailsOpen && (
          <div className={`${TEAM_SIDEBAR_META_GRID_CLASS} mt-3 border-t border-notion-border pt-3`}>
            <span>draft_team={draftTeamName.trim() || "-"}</span>
            <span>coordinator={coordinatorMemberId.trim() || "-"}</span>
            <span>workers={configuredWorkerCount}</span>
          </div>
        )}
      </div>

      {showTeamSelector && (
        <section className={`${TEAM_SIDEBAR_SECTION_CLASS} mt-4`}>
          <button
            type="button"
            className={TEAM_SIDEBAR_SECTION_TOGGLE_CLASS}
            onClick={() => toggleSection("teams")}
            aria-expanded={sectionOpen.teams}
            aria-label="Toggle teams section"
          >
            <span>Teams</span>
            <i
              className={sectionOpen.teams ? "bi bi-chevron-down" : "bi bi-chevron-right"}
              aria-hidden="true"
            />
          </button>
          {sectionOpen.teams && (
            <div className="space-y-1">
              {teams.length > 0 && (
                <div className="px-2 pb-2">
                  <TextInput
                    className="flex-1"
                    placeholder="Search teams..."
                    aria-label="Search teams"
                    value={teamFilter}
                    onChange={(event) => setTeamFilter(event.currentTarget.value)}
                    size="xs"
                    radius="md"
                    variant="unstyled"
                    classNames={{
                      input: `h-8 rounded-lg border border-notion-border/70 bg-white/68 px-3 text-[12px] text-notion-text ${TEAM_SOFT_CHROME_SHADOW_CLASS} placeholder:text-notion-text-muted focus:border-notion-border-subtle focus:bg-white`,
                      section:
                        "text-notion-text-muted",
                    }}
                    rightSection={
                      hasTeamFilter ? (
                        <CloseButton
                          aria-label="Clear filter"
                          title="Clear filter"
                          onClick={() => setTeamFilter("")}
                          size="xs"
                        />
                      ) : undefined
                    }
                  />
                </div>
              )}

              <div
                className={`teams-list flex max-h-72 min-h-0 flex-col gap-0.5 overflow-auto px-1 ${TEAM_SIDEBAR_VIRTUAL_LIST_CLASS}`}
              >
                {teams.length === 0 && (
                  <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>Create a team to begin.</p>
                )}
                {teams.length > 0 && filteredTeams.length === 0 && (
                  <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>No results found.</p>
                )}
                {filteredTeams.map((team) => {
                  const isSelected = team.id === selectedTeamId;
                  return (
                    <button
                      key={team.id}
                      type="button"
                      className={
                        isSelected
                          ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                          : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                      }
                      onClick={() => {
                        onSelectTeam(team.id);
                      }}
                      aria-current={isSelected ? "true" : undefined}
                      data-team-selected={isSelected ? "true" : "false"}
                      title={developerMode ? team.id : team.name}
                    >
                      <span className="truncate text-[13px] font-medium text-inherit">{team.name}</span>
                    </button>
                  );
                })}
              </div>

              {teams.length === 0 && (
                <div className="mt-2 px-2">
                  <button
                    type="button"
                    className="inline-flex h-8 items-center gap-2 rounded-md px-2 text-[12px] font-medium text-notion-text-muted transition hover:bg-notion-text/[0.05] hover:text-notion-text"
                    onClick={onOpenCreateTeam}
                  >
                    <i className="bi bi-plus-lg" aria-hidden="true" />
                    <span>New Team</span>
                  </button>
                </div>
              )}
            </div>
          )}
        </section>
      )}

      {selectedTeam && (
        <>
          <div className={TEAM_SUBJECT_SWITCHER_SECTION_CLASS}>
            <div className={TEAM_SUBJECT_SWITCHER_CLASS} role="tablist" aria-label="Team sidebar sections">
              {subjectPaneItems.map((item) => {
                const isSelected = activeSubjectPane === item.value;
                return (
                  <button
                    key={item.value}
                    type="button"
                    role="tab"
                    aria-selected={isSelected}
                    aria-label={item.ariaLabel}
                    title={item.label}
                    className={
                      isSelected
                        ? TEAM_SUBJECT_SWITCHER_ACTIVE_CLASS
                        : TEAM_SUBJECT_SWITCHER_IDLE_CLASS
                    }
                    onClick={() => handleSelectSubjectPane(item.value)}
                  >
                    <i className={`bi ${item.icon}`} aria-hidden="true" />
                  </button>
                );
              })}
              <button
                type="button"
                aria-label="Search workspace"
                title="Search"
                className={
                  searchDialogOpen
                    ? TEAM_SUBJECT_SWITCHER_ACTIVE_CLASS
                    : TEAM_SUBJECT_SWITCHER_IDLE_CLASS
                }
                onClick={handleOpenSearch}
              >
                <i className="bi bi-search" aria-hidden="true" />
              </button>
            </div>
          </div>
          <Modal
            opened={searchDialogOpen}
            onClose={closeSearchDialog}
            title="Search workspace"
            centered
            size="lg"
            radius="xl"
            overlayProps={NOTION_MODAL_OVERLAY_PROPS}
            classNames={{
              ...NOTION_MODAL_CLASSNAMES,
              body: "px-3 pb-3 pt-2",
            }}
          >
            <div className="flex flex-col gap-2">
              <label className="relative block">
                <span className="sr-only">Search workspace</span>
                <i
                  className="bi bi-search pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[13px] text-notion-text-muted"
                  aria-hidden="true"
                />
                <input
                  className="h-10 w-full rounded-lg border border-notion-border bg-white pl-8 pr-3 text-[13px] text-notion-text outline-none transition placeholder:text-notion-text-muted focus:border-notion-border-subtle focus:bg-white"
                  type="search"
                  value={sidebarSearchQuery}
                  onChange={(event) => setSidebarSearchQuery(event.currentTarget.value)}
                  placeholder="Search channels, tasks, or agents"
                  aria-label="Search workspace"
                  autoFocus
                />
              </label>
              <div
                className="max-h-[52vh] overflow-y-auto rounded-lg border border-notion-border-subtle bg-white p-1"
                data-team-search-dialog-results="true"
              >
                {sidebarSearchResults.length > 0 ? (
                  sidebarSearchResults.map((result, index) => {
                    const previous = sidebarSearchResults[index - 1];
                    const showSection = !previous || previous.section !== result.section;
                    return (
                      <React.Fragment key={result.key}>
                        {showSection ? (
                          <div className="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-notion-text-muted first:pt-1">
                            {result.section}
                          </div>
                        ) : null}
                        <button
                          type="button"
                          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition hover:bg-notion-hover"
                          onClick={result.onSelect}
                        >
                          <i
                            className={`bi ${result.icon} shrink-0 text-[13px] text-notion-text-muted`}
                            aria-hidden="true"
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-[13px] font-medium text-notion-text">
                              {result.label}
                            </span>
                            <span className="block truncate text-[11px] text-notion-text-muted">
                              {result.description}
                            </span>
                          </span>
                        </button>
                      </React.Fragment>
                    );
                  })
                ) : (
                  <div className="px-2 py-8 text-center text-[12px] text-notion-text-muted">
                    No results
                  </div>
                )}
              </div>
            </div>
          </Modal>

          {activeSubjectPane === "channels" && (
          <div className="mt-3 flex flex-col gap-0.5">
            <div className="mb-1 mt-3 flex items-center justify-between px-2 text-[11px] font-medium tracking-[0.01em] text-notion-text-muted">
              Channels
              {onCreateChannel ? (
                <IconButton
                  onClick={() => {
                    setShowCreateChannelForm((current) => !current);
                  }}
                  disabled={creatingChannel}
                  tone="subtle"
                  size="sm"
                  className="h-7 w-7 text-notion-text-muted hover:bg-notion-hover hover:text-notion-text"
                  title="Create channel"
                  aria-label="Create channel"
                >
                  <i className="bi bi-plus-lg" aria-hidden="true" />
                </IconButton>
              ) : null}
            </div>
            {showCreateChannelForm && onCreateChannel ? (
              <form
                className="mb-2 flex flex-col gap-2 px-2"
                onSubmit={handleCreateChannelSubmit}
              >
                <TextInput
                  aria-label="Channel ID"
                  placeholder="review"
                  value={newChannelId}
                  onChange={(event) => setNewChannelId(event.currentTarget.value)}
                  size="xs"
                />
                <TextInput
                  aria-label="Channel Description"
                  placeholder="Review lane"
                  value={newChannelDescription}
                  onChange={(event) => setNewChannelDescription(event.currentTarget.value)}
                  size="xs"
                />
                <div className="flex items-center justify-end gap-2">
                  <button
                    type="button"
                    className="inline-flex h-7 items-center rounded-md px-2 text-[11px] font-medium text-notion-text-muted transition hover:bg-notion-text/[0.05] hover:text-notion-text"
                    onClick={resetCreateChannelForm}
                    disabled={creatingChannel}
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    className="inline-flex h-7 items-center rounded-md bg-notion-text px-2.5 text-[11px] font-medium text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={!canSubmitChannelCreate}
                  >
                    Create channel
                  </button>
                </div>
              </form>
            ) : null}
            {channelItems.map((channel) => {
              const isSelected = tab === "conversation" && selectedChannelId === channel.id;
              const isDefaultChannel = channel.id === DEFAULT_TEAM_CHANNEL_ID;
              const isDeleting = deletingChannelId === channel.id;
              return (
                <div
                  key={channel.id}
                  className="group flex items-center gap-1"
                >
                  <UnstyledButton
                    className={
                      isSelected
                        ? `${TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS} flex-1`
                        : `${TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS} flex-1`
                    }
                    onClick={() => onSelectChannel(channel.id)}
                    title={channel.label}
                  >
                    <span className="min-w-0 flex-1 text-left">
                      <span className="block truncate text-[12px] font-medium">
                        {channel.label}
                      </span>
                      {channel.description ? (
                        <span className="block truncate text-[10px] text-notion-text-muted">
                          {channel.description}
                        </span>
                      ) : null}
                    </span>
                  </UnstyledButton>
                  {!isDefaultChannel && onDeleteChannel ? (
                    <IconButton
                      onClick={() => {
                        if (
                          typeof window !== "undefined" &&
                          !window.confirm(`Delete channel "${channel.label}"?`)
                        ) {
                          return;
                        }
                        void onDeleteChannel(channel.id);
                      }}
                      disabled={isDeleting || creatingChannel}
                      tone="subtle"
                      size="sm"
                      className="h-7 w-7 shrink-0 text-notion-text-muted opacity-0 transition group-hover:opacity-100 focus-visible:opacity-100 hover:bg-notion-hover hover:text-rose-600 disabled:opacity-50"
                      title={`Delete ${channel.label}`}
                      aria-label={`Delete channel ${channel.id}`}
                    >
                      <i className="bi bi-trash3" aria-hidden="true" />
                    </IconButton>
                  ) : null}
                </div>
              );
            })}
          </div>
          )}

          {activeSubjectPane === "tasks" && (
          <div className="mt-3 flex flex-col gap-0.5">
            <div className="mb-1 mt-3 flex items-center justify-between px-2 text-[11px] font-medium tracking-[0.01em] text-notion-text-muted">
              Tasks
            </div>
            <UnstyledButton
              className={
                tab === "tasks"
                  ? TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectKanban}
              title="Team kanban"
            >
              <i className="bi bi-kanban text-[14px]" aria-hidden="true" />
              <span className="min-w-0 flex-1 text-left">
                <span className="block truncate text-[12px] font-medium">Kanban</span>
                <span className="block truncate text-[10px] text-notion-text-muted">
                  {describeTeamKanban(selectedChannelLabel)}
                </span>
              </span>
            </UnstyledButton>
          </div>
          )}

          {activeSubjectPane === "agents" && (
          <section className={`${TEAM_SIDEBAR_SECTION_CLASS} mt-3`}>
            <div className="mb-1 mt-3 flex items-center justify-between px-2 text-[11px] font-medium tracking-[0.01em] text-notion-text-muted">
              Agents
            </div>
              <div className={`${TEAM_SIDEBAR_NAV_LIST_CLASS} ${TEAM_SIDEBAR_VIRTUAL_LIST_CLASS} px-1`}>
                {memberLiveStates.length === 0 && (
                  <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>No agents joined yet.</p>
                )}
                {memberLiveStates.map((member) => {
                  const lifecycle = normalizeTeamMemberLifecycle(member);
                  const workStatus = normalizeTeamMemberWorkStatus(member);
                  const isActiveMember =
                    focusedAgentMemberId === member.member_id && AGENT_FOCUS_TABS.has(tab);
                  const primaryLabel = resolveMemberPrimaryLabel(member);
                  const currentWorkLabel = resolveCurrentWorkLabel(member);
                  const memberStateLabel = formatMemberStateLabel(lifecycle, workStatus);
                  const showMemberStateLabel = shouldShowMemberStateLabel(memberStateLabel);
                  return (
                    <button
                      key={member.member_id}
                      type="button"
                      className={
                        isActiveMember
                          ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                          : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                      }
                      onClick={() => onSelectAgentTab(member.member_id, "agent_acp")}
                      title={primaryLabel}
                      data-team-member-id={member.member_id}
                    >
                      <span className="flex w-full items-center justify-between gap-1.5">
                        <span className="min-w-0 flex items-center gap-1.5">
                          <span className="relative inline-flex h-5 w-5 shrink-0 items-center justify-center">
                            <DeterministicAvatar
                              name={primaryLabel}
                              stableId={member.member_id}
                              className="h-5 w-5 border border-black/8 shadow-[0_1px_2px_rgba(15,23,42,0.08)]"
                            />
                            <span
                              className={`absolute -bottom-0.5 -right-0.5 ${TEAM_SIDEBAR_INDICATOR_DOT_CLASS} ${resolveMemberIndicatorClassName(
                                lifecycle,
                                workStatus
                              )}`}
                              aria-hidden="true"
                            />
                          </span>
                          <span className="truncate text-[11px] font-medium leading-5">
                            {primaryLabel}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-1">
                          {showMemberStateLabel && (
                            <span className="shrink-0 text-[9px] font-medium uppercase tracking-[0.08em] text-notion-text-muted/80">
                              {memberStateLabel}
                            </span>
                          )}
                        </span>
                      </span>
                      {currentWorkLabel && (
                        <span className={TEAM_SIDEBAR_WORK_CLASS}>
                          {currentWorkLabel}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
          </section>
          )}

        </>
      )}
    </aside>
  );
}

export const TeamSidebar = React.memo(TeamSidebarImpl);
TeamSidebar.displayName = "TeamSidebar";
