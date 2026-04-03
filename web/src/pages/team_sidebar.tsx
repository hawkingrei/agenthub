import React from "react";
import { CloseButton, Menu, TextInput } from "@mantine/core";
import { TeamDefinitionRecord } from "../api";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_GHOST_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_SIDEBAR_META_GRID_CLASS,
  TEAM_SIDEBAR_ROOT_CLASS,
  TEAM_SIDEBAR_SECTION_CLASS,
  TEAM_SIDEBAR_SECTION_TOGGLE_CLASS,
  TEAM_SIDEBAR_NAV_LIST_CLASS,
  TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS,
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

function formatRoleLabel(role: string): string {
  return humanizeToken(role);
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

const TEAM_WORKBENCH_SIDEBAR_ROOT_CLASS =
  "rounded-[18px] border-0 bg-transparent p-1 shadow-none";
const TEAM_WORKBENCH_SIDEBAR_PANEL_CLASS =
  "rounded-[14px] border-0 bg-transparent p-0 shadow-none";
const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS =
  "px-1 py-0.5";
const TEAM_WORKBENCH_SIDEBAR_ACTION_CLASS =
  "inline-flex items-center justify-center rounded-[10px] border border-transparent bg-transparent px-2 py-1.5 text-[12px] font-medium text-ui-text-primary shadow-none transition hover:bg-black/[0.04]";
const TEAM_WORKBENCH_SIDEBAR_ACTION_ICON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-[10px] border border-transparent bg-transparent text-ui-text-primary shadow-none transition hover:bg-black/[0.04]";
const TEAM_WORKBENCH_SIDEBAR_PICKER_ACTIVE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-[10px] bg-black/[0.055] px-2.5 py-1.5 text-left text-ui-text-primary";
const TEAM_WORKBENCH_SIDEBAR_PICKER_IDLE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-[10px] bg-transparent px-2.5 py-1.5 text-left text-ui-text-primary transition hover:bg-black/[0.03]";
const TEAM_WORKBENCH_SIDEBAR_SECTION_TOGGLE_CLASS =
  "flex w-full items-center justify-between px-1 py-1 text-left text-[11px] font-medium tracking-normal text-ui-text-muted";
const TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-1 rounded-[10px] bg-black/[0.055] px-2.5 py-1.5 text-left text-ui-text-primary transition";
const TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-1 rounded-[10px] bg-transparent px-2.5 py-1.5 text-left text-ui-text-primary transition hover:bg-black/[0.03]";
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_ACTIVE_CLASS =
  "flex w-full items-center gap-2 rounded-[10px] bg-black/[0.055] px-2.5 py-1.5 text-left text-ui-text-primary transition";
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS =
  "flex w-full items-center gap-2 rounded-[10px] bg-transparent px-2.5 py-1.5 text-left text-ui-text-primary transition hover:bg-black/[0.03]";
const TEAM_WORKBENCH_SIDEBAR_WORK_CLASS =
  "truncate pl-4 text-[11px] leading-4 text-ui-text-secondary";
const TEAM_WORKBENCH_SIDEBAR_SUMMARY_CLASS =
  "shrink-0 rounded-full bg-black/[0.05] px-1.5 py-0.5 text-[10px] font-medium leading-none text-ui-text-muted";

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

export function TeamSidebar(props: TeamSidebarProps) {
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
  const [teamActionsOpen, setTeamActionsOpen] = React.useState(false);
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
      className={`${TEAM_SIDEBAR_ROOT_CLASS} ${TEAM_WORKBENCH_SIDEBAR_ROOT_CLASS}`}
      data-team-surface="sidebar"
    >
      <div className={TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS}>
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            {showTeamSelector ? (
              <div className="truncate text-[15px] font-semibold leading-tight tracking-tight text-ui-text-primary">
                {selectedTeam?.name ?? "Select a team"}
              </div>
            ) : selectedTeam ? (
              <Menu withinPortal={false} position="bottom-start" shadow="md">
                <Menu.Target>
                  <button
                    type="button"
                    className="inline-flex max-w-full items-center gap-1 rounded-[8px] px-1 py-0.5 text-left transition hover:bg-[rgba(55,53,47,0.05)]"
                    aria-label="Open selected team menu"
                    title="Open selected team menu"
                  >
                    <span className="truncate text-[15px] font-semibold leading-tight tracking-tight text-ui-text-primary">
                      {selectedTeam.name}
                    </span>
                    <span className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-[7px] text-ui-text-muted">
                      <i className="bi bi-three-dots" aria-hidden="true" />
                    </span>
                  </button>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Label>{selectedTeam.name}</Menu.Label>
                  {selectedTeamRuntimeStatus && (
                    <Menu.Item disabled>
                      <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                        <span className="font-semibold text-ui-text-primary">Runtime</span>
                        <span className="ml-2">
                          {selectedTeamRuntimeStatus.label} · {selectedTeamRuntimeStatus.online}/
                          {selectedTeamRuntimeStatus.total} online
                        </span>
                      </div>
                    </Menu.Item>
                  )}
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                      <span className="font-semibold text-ui-text-primary">Members</span>
                      <span className="ml-2">{selectedTeamMemberCount}</span>
                    </div>
                  </Menu.Item>
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                      <span className="font-semibold text-ui-text-primary">Workers</span>
                      <span className="ml-2">{selectedTeamWorkerCount}</span>
                    </div>
                  </Menu.Item>
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                      <span className="font-semibold text-ui-text-primary">Goal</span>
                      <p className="mt-1 whitespace-pre-wrap text-ui-text-secondary">
                        {selectedTeam.description?.trim() || "No team goal description yet."}
                      </p>
                    </div>
                  </Menu.Item>
                  {developerMode && (
                    <Menu.Item disabled>
                      <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                        <span className="font-semibold text-ui-text-primary">Team ID</span>
                        <span className="ml-2 break-all">{selectedTeam.id}</span>
                      </div>
                    </Menu.Item>
                  )}
                  {(onOpenTeamMemberForge || onStartTeamRuntime || onStopTeamRuntime) && (
                    <>
                      <Menu.Divider />
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
                </Menu.Dropdown>
              </Menu>
            ) : (
              <div className="truncate text-[15px] font-semibold leading-tight tracking-tight text-ui-text-primary">
                Team workspace
              </div>
            )}
            {showTeamSelector && (
              <p className="mt-0.5 text-[12px] leading-5 text-ui-text-secondary">
                {selectedTeam
                  ? "Switch teams from the index below."
                  : "Choose an existing team or create a new one."}
              </p>
            )}
          </div>
          {showTeamSelector && (
            <div className="flex items-center gap-2 pt-0.5">
              <button
                onClick={() => {
                  void onRefreshTeams();
                }}
                disabled={busy === "refresh-teams"}
                className={`${TEAM_PANEL_REFRESH_BUTTON_CLASS} ${TEAM_WORKBENCH_SIDEBAR_ACTION_CLASS}`}
                title="Refresh teams"
                aria-label="Refresh teams"
              >
                <i className="bi bi-arrow-clockwise" aria-hidden="true" />
                <span className="hidden sm:inline">Refresh</span>
              </button>
              <div className="relative">
                <button
                  type="button"
                  className={`${TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS} ${TEAM_WORKBENCH_SIDEBAR_ACTION_ICON_CLASS}`}
                  aria-label="Open team actions"
                  title="Open team actions"
                  aria-expanded={teamActionsOpen}
                  onClick={() => setTeamActionsOpen((current) => !current)}
                >
                  <i className="bi bi-three-dots" aria-hidden="true" />
                </button>
                {teamActionsOpen && (
                  <div className="absolute right-0 top-full z-20 mt-2 flex min-w-44 flex-col gap-1 rounded-[16px] border border-ui-border bg-ui-surface p-2 shadow-lg">
                    <button
                      type="button"
                      className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} w-full justify-start rounded-[12px] border border-ui-border bg-ui-surface text-ui-text-primary`}
                      onClick={() => {
                        setTeamActionsOpen(false);
                        onOpenCreateTeam();
                      }}
                    >
                      Create Team
                    </button>
                    {developerMode && (
                      <>
                        <div className="my-1 border-t border-ui-border" />
                        <button
                          type="button"
                          className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} w-full justify-start rounded-[12px] border border-ui-border bg-ui-surface text-ui-text-primary`}
                          onClick={() => {
                            setTeamActionsOpen(false);
                            setTeamDetailsOpen((current) => !current);
                          }}
                        >
                          {teamDetailsOpen ? "Hide Team Details" : "Show Team Details"}
                        </button>
                      </>
                    )}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
        {showTeamSelector && teamDetailsOpen && (
          <div className={`${TEAM_SIDEBAR_META_GRID_CLASS} mt-3 border-t border-ui-border pt-3 text-ui-text-muted`}>
            <span>draft_team={draftTeamName.trim() || "-"}</span>
            <span>leader={leaderMemberId.trim() || "-"}</span>
            <span>workers={configuredWorkerCount}</span>
          </div>
        )}
      </div>

      {showTeamSelector && (
        <section className={`${TEAM_SIDEBAR_SECTION_CLASS} mt-3`}>
          <button
            type="button"
            className={`${TEAM_SIDEBAR_SECTION_TOGGLE_CLASS} ${TEAM_WORKBENCH_SIDEBAR_SECTION_TOGGLE_CLASS}`}
            onClick={() => toggleSection("teams")}
            aria-expanded={sectionOpen.teams}
            aria-label="Toggle teams section"
          >
            <span>{`Teams ${teams.length}`}</span>
            <i
              className={sectionOpen.teams ? "bi bi-chevron-down" : "bi bi-chevron-right"}
              aria-hidden="true"
            />
          </button>
          {sectionOpen.teams && (
            <div className={`${TEAM_WORKBENCH_SIDEBAR_PANEL_CLASS} mt-1.5`}>
              {teams.length > 0 && (
                <div className="teams-filter flex items-start gap-2">
                  <TextInput
                    className="flex-1"
                    placeholder="Filter teams by name or id"
                    aria-label="Filter teams"
                    value={teamFilter}
                    onChange={(event) => setTeamFilter(event.currentTarget.value)}
                    size="sm"
                    radius="md"
                    rightSection={
                      hasTeamFilter ? (
                        <CloseButton
                          aria-label="Clear team filter"
                          title="Clear team filter"
                          onClick={() => setTeamFilter("")}
                          size="sm"
                        />
                      ) : undefined
                    }
                  />
                </div>
              )}

              <div className="teams-list mt-2 flex max-h-72 min-h-0 flex-col gap-1.5 overflow-auto">
                {teams.length === 0 && <p className={TEAM_MUTED_TEXT_CLASS}>No teams yet.</p>}
                {teams.length > 0 && filteredTeams.length === 0 && (
                  <p className={TEAM_MUTED_TEXT_CLASS}>No teams match current filter.</p>
                )}
                {hasTeamFilter && filteredTeams.length > 0 && (
                  <p className={`${TEAM_MUTED_TEXT_CLASS} mono`}>{`filtered=${filteredTeams.length} total=${teams.length}`}</p>
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
                          ? TEAM_WORKBENCH_SIDEBAR_PICKER_ACTIVE_CLASS
                          : TEAM_WORKBENCH_SIDEBAR_PICKER_IDLE_CLASS
                      }
                      onClick={() => {
                        onSelectTeam(team.id);
                      }}
                      aria-current={isSelected ? "true" : undefined}
                      data-team-selected={isSelected ? "true" : "false"}
                      title={developerMode ? team.id : team.name}
                    >
                      <span className="flex w-full items-start justify-between gap-2">
                        <span className={TEAM_LIST_ITEM_TITLE_CLASS}>
                          {team.name}
                        </span>
                      </span>
                      {summaryLabel && (
                        <span className={`${TEAM_LIST_ITEM_META_CLASS} text-ui-text-muted`}>
                          {summaryLabel}
                        </span>
                      )}
                      {developerMode && (
                        <span className={`${TEAM_LIST_ITEM_META_CLASS} text-ui-text-muted/90`}>
                          {team.id}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>

              {teams.length === 0 && (
                <div className="mt-3 flex flex-wrap gap-2">
                  <button
                    type="button"
                    className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} rounded-[12px] border border-ui-border bg-ui-surface text-ui-text-primary shadow-sm`}
                    onClick={onOpenCreateTeam}
                  >
                    Create Team
                  </button>
                </div>
              )}
            </div>
          )}
        </section>
      )}

      {selectedTeam && (
        <>
          <div className="mt-3 flex flex-col gap-1.5">
            <div className="px-1 pt-1 text-[11px] font-medium text-ui-text-muted">
              Workflow
            </div>
            <button
              type="button"
              className={
                tab === "conversation"
                  ? TEAM_WORKBENCH_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectConversation}
              title="Shared channel for human requests, planning discussion, and team-visible progress updates."
            >
              <i className="bi bi-hash text-[13px] text-ui-text-muted" aria-hidden="true" />
              <span className="truncate text-[13px] font-medium text-ui-text-primary"># all</span>
            </button>
            <button
              type="button"
              className={
                tab === "tasks"
                  ? TEAM_WORKBENCH_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectKanban}
              title="Canonical system-managed Team tasks and execution state."
            >
              <i className="bi bi-kanban text-[13px] text-ui-text-muted" aria-hidden="true" />
              <span className="truncate text-[13px] font-medium text-ui-text-primary">Kanban</span>
            </button>
          </div>

          <section className={`${TEAM_SIDEBAR_SECTION_CLASS} mt-3`}>
            <button
              type="button"
              className={`${TEAM_SIDEBAR_SECTION_TOGGLE_CLASS} ${TEAM_WORKBENCH_SIDEBAR_SECTION_TOGGLE_CLASS}`}
              onClick={() => toggleSection("agents")}
              aria-expanded={sectionOpen.agents}
              aria-label="Toggle agents section"
            >
              <span>{`Agents ${memberLiveStates.length}`}</span>
              <i
                className={sectionOpen.agents ? "bi bi-chevron-down" : "bi bi-chevron-right"}
                aria-hidden="true"
              />
            </button>
            {sectionOpen.agents && (
              <div className={TEAM_SIDEBAR_NAV_LIST_CLASS}>
                {memberLiveStates.length === 0 && (
                  <p className={TEAM_MUTED_TEXT_CLASS}>No members found in current team spec.</p>
                )}
                {memberLiveStates.map((member) => {
                  const lifecycle = normalizeTeamMemberLifecycle(member);
                  const workStatus = normalizeTeamMemberWorkStatus(member);
                  const isActiveMember =
                    focusedAgentMemberId === member.member_id && AGENT_FOCUS_TABS.has(tab);
                  const primaryLabel = resolveMemberPrimaryLabel(member);
                  const currentWorkLabel = resolveCurrentWorkLabel(member);
                  const memberStateLabel = formatMemberStateLabel(lifecycle, workStatus);
                  return (
                    <button
                      key={member.member_id}
                      type="button"
                      className={
                        isActiveMember
                          ? TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS
                          : TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS
                      }
                      onClick={() => onSelectAgentTab(member.member_id, "agent_acp")}
                      title={
                        [
                          primaryLabel,
                          formatRoleLabel(member.role),
                          memberStateLabel,
                          developerMode ? member.member_id : null,
                          currentWorkLabel,
                        ]
                          .filter((value) => value && value.trim().length > 0)
                          .join(" · ") || member.member_id
                      }
                    >
                      <span className="flex w-full items-start justify-between gap-2">
                        <span className="min-w-0 flex items-center gap-2">
                          <span
                            className={`mt-[5px] h-2 w-2 shrink-0 rounded-full ${resolveMemberIndicatorClassName(
                              lifecycle,
                              workStatus
                            )}`}
                            aria-hidden="true"
                          />
                          <span className="truncate text-[13px] font-medium text-ui-text-primary">
                            {primaryLabel}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-1.5">
                          <span className={TEAM_WORKBENCH_SIDEBAR_SUMMARY_CLASS}>
                            {memberStateLabel}
                          </span>
                          {(member.pending_inbox_count ?? 0) > 0 && (
                            <span className="shrink-0 rounded-full bg-black/[0.06] px-1.5 py-0.5 text-[10px] font-medium text-ui-text-primary">
                              {member.pending_inbox_count}
                            </span>
                          )}
                        </span>
                      </span>
                      {currentWorkLabel && (
                        <span
                          className={`${TEAM_WORKBENCH_SIDEBAR_WORK_CLASS} ${
                            isActiveMember ? "text-ui-text-primary" : ""
                          }`}
                        >
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
