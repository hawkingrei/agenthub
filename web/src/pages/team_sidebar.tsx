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
  selectedTeamMemberCount?: number;
  selectedTeamHasConfiguredMembers?: boolean;
  teamMemberSummaryByTeamId: Map<string, TeamMemberSummary>;
  memberLiveStates: TeamMemberLiveState[];
  focusedAgentMemberId: string;
  tab: TeamTab;
  onSelectTeam: (teamId: string) => void;
  onSelectConversation: () => void;
  onSelectKanban: () => void;
  onSelectAgentTab: (memberId: string, tab: TeamTab) => void;
  onSelectUtilityTab: (tab: TeamTab) => void;
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

export function formatTeamMemberSummary(summary?: TeamMemberSummary): string | null {
  if (!summary) {
    return null;
  }
  const parts = [`${summary.total} members`, `${summary.active} active`];
  if (summary.inactive > 0) {
    parts.push(`${summary.inactive} idle`);
  }
  if (summary.missing > 0) {
    parts.push(`${summary.missing} missing`);
  }
  return parts.join(" · ");
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

function resolveCurrentWorkLabel(member: TeamMemberLiveState): string | null {
  const currentWork = member.current_work?.trim();
  if (!currentWork || currentWork === NO_ACTIVE_RUN_CONTEXT) {
    return null;
  }
  return currentWork;
}

const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS = "px-2 py-1.5";
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS = TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS;

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
    selectedTeamMemberCount = 0,
    selectedTeamHasConfiguredMembers = false,
    teamMemberSummaryByTeamId,
    memberLiveStates,
    focusedAgentMemberId,
    tab,
    onSelectTeam,
    onSelectConversation,
    onSelectKanban,
    onSelectAgentTab,
    onOpenTeamMemberForge,
    onStartTeamRuntime,
    onStopTeamRuntime,
  } = props;
  const [teamFilter, setTeamFilter] = React.useState("");
  const [teamDetailsOpen, setTeamDetailsOpen] = React.useState(false);
  const [sectionOpen, setSectionOpen] = React.useState<Record<TeamSidebarSection, boolean>>({
    teams: true,
    agents: true,
  });
  const deferredTeamFilter = React.useDeferredValue(teamFilter);
  const normalizedTeamFilter = deferredTeamFilter.trim().toLowerCase();
  const selectedTeamWorkerCount = React.useMemo(
    () => memberLiveStates.filter((member) => member.role === "worker").length,
    [memberLiveStates]
  );
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

  const toggleSection = React.useCallback((section: TeamSidebarSection) => {
    setSectionOpen((current) => ({
      ...current,
      [section]: !current[section],
    }));
  }, []);

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
              <Menu
                position="bottom-start"
                {...NOTION_FLOATING_MENU_PROPS}
              >
                <Menu.Target>
                  <UnstyledButton
                    className="inline-flex max-w-full items-center gap-1 rounded-md px-1 py-1 text-left transition hover:bg-[rgba(55,53,47,0.05)]"
                    aria-label="Team"
                    title="Team"
                  >
                    <span className="truncate text-[15px] font-semibold tracking-tight text-notion-text">
                      {selectedTeam.name}
                    </span>
                    <span className="inline-flex h-4 w-4 shrink-0 items-center justify-center text-[11px] text-notion-text-muted">
                      <i className="bi bi-chevron-down" aria-hidden="true" />
                    </span>
                  </UnstyledButton>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Label>{selectedTeam.name}</Menu.Label>
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
            {showTeamSelector && teams.length > 0 && (
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

            <div className="teams-list flex max-h-72 min-h-0 flex-col gap-0.5 overflow-auto px-1">
              {teams.length === 0 && (
                <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>Create a team to begin.</p>
              )}
              {showTeamSelector && teams.length > 0 && filteredTeams.length === 0 && (
                <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>No results found.</p>
              )}
              {filteredTeams.map((team) => {
                const summary = teamMemberSummaryByTeamId.get(team.id);
                const summaryLabel = formatTeamMemberSummary(summary);
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

            {showTeamSelector && teams.length === 0 && (
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

      {selectedTeam && (
        <>
          <div className="mt-4 flex flex-col gap-0.5">
            <div className="mb-1 mt-3 flex items-center justify-between px-2 text-[11px] font-medium tracking-[0.01em] text-notion-text-muted">
              Channels
            </div>
            <button
              type="button"
              className={
                tab === "conversation"
                  ? TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectConversation}
              title="Shared channel"
            >
              <span
                className="inline-flex h-4 w-4 shrink-0 items-center justify-center text-[12px] font-semibold leading-none text-notion-text-muted/80"
                aria-hidden="true"
              >
                #
              </span>
              <span className="truncate text-[12px] font-medium"># all</span>
            </button>
            <button
              type="button"
              className={
                tab === "tasks"
                  ? TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectKanban}
              title="Team kanban"
              >
                <i className="bi bi-kanban text-[14px]" aria-hidden="true" />
                <span className="truncate text-[12px] font-medium">Kanban</span>
            </button>
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
              <div className={`${TEAM_SIDEBAR_NAV_LIST_CLASS} px-1`}>
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
                  const showMemberStateLabel = memberStateLabel !== "Offline";
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
