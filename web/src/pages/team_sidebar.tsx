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
  TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS,
  TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS,
  TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS,
  TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS,
  TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS,
  TEAM_SIDEBAR_WORK_CLASS,
  TEAM_SIDEBAR_BADGE_CLASS,
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

const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS = "px-3 py-2";
const TEAM_WORKBENCH_SIDEBAR_PICKER_ACTIVE_CLASS = TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_PICKER_IDLE_CLASS = TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_SECTION_TOGGLE_CLASS = TEAM_SIDEBAR_SECTION_TOGGLE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS = TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS = TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_ACTIVE_CLASS = TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS = TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS;
const TEAM_WORKBENCH_SIDEBAR_WORK_CLASS = TEAM_SIDEBAR_WORK_CLASS;
const TEAM_WORKBENCH_SIDEBAR_SUMMARY_CLASS = TEAM_SIDEBAR_BADGE_CLASS;

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
      className={TEAM_SIDEBAR_ROOT_CLASS}
      data-team-surface="sidebar"
    >
      <div className={TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS}>
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            {showTeamSelector ? (
              <div className="truncate text-[15px] font-bold tracking-tight text-notion-text">
                {selectedTeam?.name ?? "Select a team"}
              </div>
            ) : selectedTeam ? (
              <Menu withinPortal={false} position="bottom-start" shadow="md">
                <Menu.Target>
                  <button
                    type="button"
                    className="inline-flex max-w-full items-center gap-1 rounded-md px-2 py-1 text-left transition hover:bg-notion-hover"
                    aria-label="Open selected team menu"
                    title="Open selected team menu"
                  >
                    <span className="truncate text-[15px] font-bold tracking-tight text-notion-text">
                      {selectedTeam.name}
                    </span>
                    <span className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-sm text-notion-text-muted">
                      <i className="bi bi-three-dots" aria-hidden="true" />
                    </span>
                  </button>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Label>{selectedTeam.name}</Menu.Label>
                  {selectedTeamRuntimeStatus && (
                    <Menu.Item disabled>
                      <div className="min-w-[240px] text-[12px] leading-5 text-notion-text-muted">
                        <span className="font-bold text-notion-text">Runtime</span>
                        <span className="ml-2">
                          {selectedTeamRuntimeStatus.label} · {selectedTeamRuntimeStatus.online}/
                          {selectedTeamRuntimeStatus.total} online
                        </span>
                      </div>
                    </Menu.Item>
                  )}
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-notion-text-muted">
                      <span className="font-bold text-notion-text">Members</span>
                      <span className="ml-2">{selectedTeamMemberCount}</span>
                    </div>
                  </Menu.Item>
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-notion-text-muted">
                      <span className="font-bold text-notion-text">Workers</span>
                      <span className="ml-2">{selectedTeamWorkerCount}</span>
                    </div>
                  </Menu.Item>
                  <Menu.Item disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-notion-text-muted">
                      <span className="font-bold text-notion-text">Goal</span>
                      <p className="mt-1 whitespace-pre-wrap text-notion-text">
                        {selectedTeam.description?.trim() || "No team goal description yet."}
                      </p>
                    </div>
                  </Menu.Item>
                  {developerMode && (
                    <Menu.Item disabled>
                      <div className="min-w-[240px] text-[12px] leading-5 text-notion-text-muted">
                        <span className="font-bold text-notion-text">Team ID</span>
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
              <div className="truncate text-[15px] font-bold tracking-tight text-notion-text">
                Team workspace
              </div>
            )}
            {showTeamSelector && (
              <p className="mt-0.5 text-[12px] leading-relaxed text-notion-text-muted">
                {selectedTeam
                  ? "Switch teams from the index below."
                  : "Choose an existing team or create a new one."}
              </p>
            )}
          </div>
          {showTeamSelector && (
            <div className="flex items-center gap-1.5 pt-0.5">
              <button
                onClick={() => {
                  void onRefreshTeams();
                }}
                disabled={busy === "refresh-teams"}
                className={`${TEAM_PANEL_REFRESH_BUTTON_CLASS} inline-flex h-8 w-8 items-center justify-center rounded-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text`}
                title="Refresh teams"
                aria-label="Refresh teams"
              >
                <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              </button>
              <div className="relative">
                <button
                  type="button"
                  className={`${TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS} h-8 w-8`}
                  aria-label="Open team actions"
                  title="Open team actions"
                  aria-expanded={teamActionsOpen}
                  onClick={() => setTeamActionsOpen((current) => !current)}
                >
                  <i className="bi bi-three-dots" aria-hidden="true" />
                </button>
                {teamActionsOpen && (
                  <div className="absolute right-0 top-full z-20 mt-2 flex min-w-44 flex-col gap-1 rounded-lg border border-notion-border bg-white p-1.5 shadow-xl backdrop-blur-md">
                    <button
                      type="button"
                      className="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-[13px] font-medium text-notion-text transition hover:bg-notion-hover"
                      onClick={() => {
                        setTeamActionsOpen(false);
                        onOpenCreateTeam();
                      }}
                    >
                      <i className="bi bi-plus-lg" aria-hidden="true" />
                      <span>Create Team</span>
                    </button>
                    {developerMode && (
                      <>
                        <div className="my-1 border-t border-notion-border" />
                        <button
                          type="button"
                          className="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-[13px] font-medium text-notion-text transition hover:bg-notion-hover"
                          onClick={() => {
                            setTeamActionsOpen(false);
                            setTeamDetailsOpen((current) => !current);
                          }}
                        >
                          <i className="bi bi-info-circle" aria-hidden="true" />
                          <span>{teamDetailsOpen ? "Hide Team Details" : "Show Team Details"}</span>
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
            <span>{`Teams · ${teams.length}`}</span>
            <i
              className={sectionOpen.teams ? "bi bi-chevron-down" : "bi bi-chevron-right"}
              aria-hidden="true"
            />
          </button>
          {sectionOpen.teams && (
            <div className="mt-1.5 space-y-1">
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
                    variant="filled"
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
                {teams.length === 0 && <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>No teams yet.</p>}
                {teams.length > 0 && filteredTeams.length === 0 && (
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
                      <span className="flex w-full items-center justify-between gap-2">
                        <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} font-medium`}>
                          {team.name}
                        </span>
                      </span>
                      {summaryLabel && (
                        <span className={TEAM_LIST_ITEM_META_CLASS}>
                          {summaryLabel}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>

              {teams.length === 0 && (
                <div className="mt-2 px-2">
                  <button
                    type="button"
                    className="flex w-full items-center gap-2 rounded-md border border-notion-border bg-white px-3 py-1.5 text-[13px] font-medium text-notion-text transition hover:bg-notion-hover"
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
            <div className="px-3 pb-1 text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">
              Workflow
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
              <i className="bi bi-hash text-[14px]" aria-hidden="true" />
              <span className="truncate text-[13px]"># all</span>
            </button>
            <button
              type="button"
              className={
                tab === "tasks"
                  ? TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_WORKFLOW_IDLE_CLASS
              }
              onClick={onSelectKanban}
              title="Kanban"
            >
              <i className="bi bi-kanban text-[14px]" aria-hidden="true" />
              <span className="truncate text-[13px]">Kanban</span>
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
              <span>{`Agents · ${memberLiveStates.length}`}</span>
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
                          <span className="truncate text-[13px] font-medium">
                            {primaryLabel}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-1.5">
                          <span className={TEAM_SIDEBAR_BADGE_CLASS}>
                            {memberStateLabel}
                          </span>
                          {(member.pending_inbox_count ?? 0) > 0 && (
                            <span className="shrink-0 rounded-sm bg-notion-accent text-white px-1 py-0.5 text-[10px] font-bold leading-none shadow-sm">
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
