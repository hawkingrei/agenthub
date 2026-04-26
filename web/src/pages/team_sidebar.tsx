import React from "react";
import { CloseButton, Menu, TextInput, UnstyledButton } from "@mantine/core";
import { TeamDefinitionRecord } from "../api";
import { NOTION_FLOATING_MENU_PROPS } from "../ui/floating_surfaces";
import { IconButton } from "../ui/primitives";
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

type TeamMemberSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

type TeamSidebarProps = {
  showTeamSelector?: boolean;
  developerMode: boolean;
  busy: string | null;
  onRefreshTeams: () => Promise<void> | void;
  onOpenCreateTeam: () => void;
  draftTeamName: string;
  leaderMemberId: string;
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
  selectedChannelId?: TeamChannelItem["id"];
  focusedAgentMemberId: string;
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
  onSelectAgentTab: (memberId: string, tab: TeamTab) => void;
  onOpenTeamMemberForge?: () => void;
  onStartTeamRuntime?: () => void;
  onStopTeamRuntime?: () => void;
};

const AGENT_FOCUS_TABS = new Set<TeamTab>(["agent_acp", "member_console", "mailbox"]);
type TeamSidebarSection = "teams" | "agents";
const NO_ACTIVE_RUN_CONTEXT = "No active run context.";

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
    if (lifecycle === "working") {
      return "Online";
    }
    return formatLifecycleLabel(lifecycle);
  }
  return formatLifecycleLabel(lifecycle);
}

function shouldShowMemberStateLabel(label: string): boolean {
  return label !== "Online" && label !== "Offline";
}

function resolveCurrentWorkLabel(member: TeamMemberLiveState): string | null {
  const currentWork = member.current_work?.trim();
  if (!currentWork || currentWork === NO_ACTIVE_RUN_CONTEXT) {
    return null;
  }
  return currentWork;
}

function resolveMemberNodeSummary(nodeId: string | null | undefined): {
  badge: "local" | "remote";
  label: string;
} | null {
  const normalized = nodeId?.trim() || null;
  if (!normalized) {
    return null;
  }
  if (normalized.toLowerCase() === "main") {
    return {
      badge: "local",
      label: "main",
    };
  }
  return {
    badge: "remote",
    label: normalized,
  };
}

const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS = "px-2 py-1.5";
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS = TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS;
const TEAM_SIDEBAR_VIRTUAL_LIST_CLASS =
  "[content-visibility:auto] [contain-intrinsic-size:1px_320px]";

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
    developerMode,
    busy,
    onRefreshTeams,
    onOpenCreateTeam,
    draftTeamName,
    leaderMemberId,
    configuredWorkerCount,
    teams,
    selectedTeam,
    selectedTeamId,
    selectedTeamRuntimeStatus,
    selectedTeamHasConfiguredMembers = false,
    memberLiveStates,
    memberTargetNodeById = {},
    channelItems = DEFAULT_TEAM_CHANNEL_ITEMS,
    selectedChannelId = "all",
    focusedAgentMemberId,
    tab,
    onSelectTeam,
    onBackToSelector,
    onSelectChannel,
    onCreateChannel,
    onDeleteChannel,
    creatingChannel = false,
    deletingChannelId = null,
    onSelectKanban,
    onSelectAgentTab,
    onOpenTeamMemberForge,
    onStartTeamRuntime,
    onStopTeamRuntime,
  } = props;
  const [teamFilter, setTeamFilter] = React.useState("");
  const [teamDetailsOpen, setTeamDetailsOpen] = React.useState(false);
  const [showCreateChannelForm, setShowCreateChannelForm] = React.useState(false);
  const [newChannelId, setNewChannelId] = React.useState("");
  const [newChannelDescription, setNewChannelDescription] = React.useState("");
  const [sectionOpen, setSectionOpen] = React.useState<Record<TeamSidebarSection, boolean>>({
    teams: true,
    agents: true,
  });
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

  const toggleSection = React.useCallback((section: TeamSidebarSection) => {
    setSectionOpen((current) => ({
      ...current,
      [section]: !current[section],
    }));
  }, []);
  const canSubmitChannelCreate = newChannelId.trim().length > 0 && !creatingChannel;
  const resetCreateChannelForm = React.useCallback(() => {
    setShowCreateChannelForm(false);
    setNewChannelId("");
    setNewChannelDescription("");
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
              <div className="flex min-w-0 items-center gap-1">
                <Menu
                  position="bottom-start"
                  {...NOTION_FLOATING_MENU_PROPS}
                >
                  <Menu.Target>
                    <UnstyledButton
                      className="inline-flex min-w-0 max-w-full items-center gap-1 rounded-md px-1 py-1 text-left transition hover:bg-[rgba(55,53,47,0.05)]"
                      aria-label={`Switch teams from ${selectedTeam.name}`}
                      title="Switch teams"
                    >
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
                      className={`${TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS} h-7 w-7 shrink-0`}
                      aria-label={`Open controls for ${selectedTeam.name}`}
                      title={`Open controls for ${selectedTeam.name}`}
                    >
                      <i className="bi bi-chevron-down" aria-hidden="true" />
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
                    {(onOpenTeamMemberForge || onStartTeamRuntime || onStopTeamRuntime) && (
                      <>
                        {onOpenTeamMemberForge && (
                          <Menu.Item
                            leftSection={<i className="bi bi-person-plus" aria-hidden="true" />}
                            onClick={onOpenTeamMemberForge}
                          >
                            Add Agent
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
                        {(onOpenTeamMemberForge || onStartTeamRuntime || onStopTeamRuntime) && (
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
                    className={`${TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS} h-8 w-8`}
                    aria-label="Open team actions"
                    title="Open team actions"
                  >
                    <i className="bi bi-three-dots" aria-hidden="true" />
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
            <span>leader={leaderMemberId.trim() || "-"}</span>
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
                    className="inline-flex h-8 items-center gap-2 rounded-md px-2 text-[12px] font-medium text-notion-text-muted transition hover:bg-[rgba(55,53,47,0.05)] hover:text-notion-text"
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
          <div className="mt-4 flex flex-col gap-0.5">
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
                    className="inline-flex h-7 items-center rounded-md px-2 text-[11px] font-medium text-notion-text-muted transition hover:bg-[rgba(55,53,47,0.05)] hover:text-notion-text"
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

          <div className="mt-4 flex flex-col gap-0.5">
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

          <section className={`${TEAM_SIDEBAR_SECTION_CLASS} mt-4`}>
            <button
              type="button"
              className={TEAM_SIDEBAR_SECTION_TOGGLE_CLASS}
              onClick={() => toggleSection("agents")}
              aria-expanded={sectionOpen.agents}
              aria-label="Toggle agents section"
            >
              <span>Agents</span>
              <i
                className={sectionOpen.agents ? "bi bi-chevron-down" : "bi bi-chevron-right"}
                aria-hidden="true"
              />
            </button>
            {sectionOpen.agents && (
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
                  const nodeSummary = resolveMemberNodeSummary(
                    memberTargetNodeById[member.member_id]
                  );
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
                    >
                      <span className="flex w-full items-center justify-between gap-2">
                        <span className="min-w-0 flex items-center gap-2">
                          <span
                            className={`${TEAM_SIDEBAR_INDICATOR_DOT_CLASS} ${resolveMemberIndicatorClassName(
                              lifecycle,
                              workStatus
                            )}`}
                            aria-hidden="true"
                          />
                          <span className="truncate text-[12px] font-medium">
                            {primaryLabel}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-1.5">
                          {nodeSummary && (
                            <span
                              className={
                                nodeSummary.badge === "local"
                                  ? "inline-flex items-center rounded-full border border-emerald-200 bg-emerald-50 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.08em] text-emerald-700"
                                  : "inline-flex items-center rounded-full border border-sky-200 bg-sky-50 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.08em] text-sky-700"
                              }
                              title={`node: ${nodeSummary.label}`}
                            >
                              {nodeSummary.badge}
                            </span>
                          )}
                          {showMemberStateLabel && (
                            <span className="shrink-0 text-[10px] font-medium text-notion-text-muted">
                              {memberStateLabel}
                            </span>
                          )}
                          {(member.pending_inbox_count ?? 0) > 0 && (
                            <span className="inline-flex min-w-[18px] items-center justify-center rounded-full bg-[rgba(55,53,47,0.06)] px-1.5 py-0.5 text-[10px] font-medium leading-none text-notion-text-muted">
                              {member.pending_inbox_count}
                            </span>
                          )}
                        </span>
                      </span>
                      {currentWorkLabel && (
                        <span className={TEAM_SIDEBAR_WORK_CLASS}>
                          {currentWorkLabel}
                        </span>
                      )}
                      {nodeSummary && (
                        <span className={TEAM_SIDEBAR_WORK_CLASS}>
                          {`node=${nodeSummary.label}`}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            )}
          </section>
        </>
      )}
    </aside>
  );
}

export const TeamSidebar = React.memo(TeamSidebarImpl);
TeamSidebar.displayName = "TeamSidebar";
