import React from "react";
import { CloseButton, Menu, TextInput } from "@mantine/core";
import { TeamDefinitionRecord } from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
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

function resolveWorkTone(status: ReturnType<typeof normalizeTeamMemberWorkStatus>): StatusTone {
  if (status === "working") return "active";
  if (status === "pending") return "warning";
  if (status === "blocked") return "danger";
  if (status === "done") return "active";
  if (status === "idle") return "inactive";
  return "neutral";
}

function formatWorkLabel(status: ReturnType<typeof normalizeTeamMemberWorkStatus>): string {
  if (status === "no_run") {
    return "no run";
  }
  return status;
}

function resolveMemberPrimaryLabel(member: TeamMemberLiveState): string {
  const agentName = member.agent_name?.trim();
  if (agentName) {
    return agentName;
  }
  return member.member_id;
}

function formatTeamMemberSummary(summary?: TeamMemberSummary): string | null {
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

const TEAM_WORKBENCH_SIDEBAR_ROOT_CLASS =
  "rounded-[18px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.98)_0%,rgba(244,241,233,0.96)_100%)] p-2.5 shadow-sm";
const TEAM_WORKBENCH_SIDEBAR_PANEL_CLASS =
  "rounded-[14px] border border-ui-border bg-ui-surface/80 p-1 shadow-sm";
const TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS =
  "rounded-[14px] border border-ui-border bg-ui-surface/85 px-2.5 py-2 shadow-sm";
const TEAM_WORKBENCH_SIDEBAR_ACTION_CLASS =
  "inline-flex items-center justify-center rounded-[10px] border border-ui-border bg-ui-surface px-2.5 py-1.5 text-[12px] font-semibold text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft";
const TEAM_WORKBENCH_SIDEBAR_ACTION_ICON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-[10px] border border-ui-border bg-ui-surface text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft";
const TEAM_WORKBENCH_SIDEBAR_PICKER_ACTIVE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-0.5 rounded-[10px] border border-ui-border-emphasis bg-ui-surface px-2.5 py-1.5 text-left text-ui-text-primary shadow-sm";
const TEAM_WORKBENCH_SIDEBAR_PICKER_IDLE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-0.5 rounded-[10px] border border-transparent bg-transparent px-2.5 py-1.5 text-left text-ui-text-primary transition hover:border-ui-border/70 hover:bg-ui-surface";
const TEAM_WORKBENCH_SIDEBAR_SECTION_TOGGLE_CLASS =
  "flex w-full items-center justify-between border-b border-ui-border px-0 py-1 text-left text-[10px] font-semibold uppercase tracking-[0.16em] text-ui-text-muted";
const TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-0.5 rounded-[10px] border border-ui-border bg-ui-surface px-2.5 py-1.5 text-left text-ui-text-primary shadow-sm transition";
const TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-0.5 rounded-[10px] border border-transparent bg-transparent px-2.5 py-1.5 text-left text-ui-text-primary transition hover:bg-ui-surface";
const TEAM_WORKBENCH_SIDEBAR_META_CLASS =
  "text-[11px] font-medium uppercase tracking-[0.14em] text-ui-text-muted";

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
  const normalizedTeamFilter = teamFilter.trim().toLowerCase();
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
    <aside className={`${TEAM_SIDEBAR_ROOT_CLASS} ${TEAM_WORKBENCH_SIDEBAR_ROOT_CLASS}`}>
      <div className={TEAM_WORKBENCH_SIDEBAR_HEADER_CLASS}>
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-ui-text-muted">
              {showTeamSelector ? "Team Selector" : "Team"}
            </p>
            {showTeamSelector ? (
              <div className="mt-1 truncate text-[15px] font-semibold leading-tight text-ui-text-primary">
                {selectedTeam?.name ?? "Select a team"}
              </div>
            ) : selectedTeam ? (
              <Menu withinPortal={false} position="bottom-start" shadow="md">
                <Menu.Target>
                  <button
                    type="button"
                    className="mt-1 inline-flex max-w-full items-center gap-1.5 rounded-full border border-ui-border bg-white/80 px-3 py-1.5 text-left text-[15px] font-semibold leading-tight tracking-tight text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-white"
                    aria-label="Open selected team menu"
                  >
                    <span className="truncate">{selectedTeam.name}</span>
                    <i
                      className="bi bi-chevron-down text-[11px] text-ui-text-muted"
                      aria-hidden="true"
                    />
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
              <div className="mt-1 truncate text-[15px] font-semibold leading-tight text-ui-text-primary">
                Team workspace
              </div>
            )}
            {showTeamSelector && (
              <p className="mt-1 text-[12px] leading-5 text-ui-text-secondary">
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
            <div className={`${TEAM_WORKBENCH_SIDEBAR_PANEL_CLASS} mt-2`}>
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
                        {isSelected && (
                          <span className="shrink-0 rounded-full border border-ui-border bg-ui-surface-soft px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted">
                            Current
                          </span>
                        )}
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
            <div className="px-1 text-[10px] font-semibold uppercase tracking-[0.16em] text-ui-text-muted">
              Channel
            </div>
            <button
              type="button"
              className={
                tab === "conversation"
                  ? TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS
              }
              onClick={onSelectConversation}
            >
              <span className="text-[13px] font-semibold text-ui-text-primary"># all</span>
            </button>
            <button
              type="button"
              className={
                tab === "tasks"
                  ? TEAM_WORKBENCH_SIDEBAR_NAV_ACTIVE_CLASS
                  : TEAM_WORKBENCH_SIDEBAR_NAV_IDLE_CLASS
              }
              onClick={onSelectKanban}
            >
              <span className="text-[13px] font-semibold text-ui-text-primary">Kanban</span>
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
                  const memberMeta = Array.from(
                    new Set(
                      [
                        primaryLabel !== member.member_id ? member.member_id : null,
                        member.role,
                        lifecycle,
                      ].filter(
                        (value): value is string => Boolean(value && value !== "unknown")
                      )
                    )
                  ).join(" · ");
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
                        [primaryLabel, developerMode ? member.member_id : null, member.current_work]
                          .filter((value) => value && value.trim().length > 0)
                          .join(" · ") || member.member_id
                      }
                    >
                      <span className="flex w-full items-start justify-between gap-2">
                        <span className="truncate text-[13px] font-semibold text-ui-text-primary">
                          {primaryLabel}
                        </span>
                        <span className="flex shrink-0 items-center gap-1.5">
                          <StatusBadge
                            label={formatWorkLabel(workStatus)}
                            tone={resolveWorkTone(workStatus)}
                            className="text-[10px] uppercase tracking-[0.08em]"
                            title={`run=${member.run_status} step=${member.step_status}`}
                          />
                          {(member.pending_inbox_count ?? 0) > 0 && (
                            <span className="shrink-0 rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-[10px] font-semibold text-ui-text-primary">
                              {member.pending_inbox_count}
                            </span>
                          )}
                        </span>
                      </span>
                      {developerMode && memberMeta && (
                        <span className={TEAM_WORKBENCH_SIDEBAR_META_CLASS}>{memberMeta}</span>
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
