import React from "react";
import { TeamDefinitionRecord } from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_PANEL_INPUT_CLASS,
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
  TEAM_SIDEBAR_NAV_ITEM_META_CLASS,
  TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS,
  TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS,
  TEAM_SIDEBAR_SWITCHER_PANEL_CLASS,
  TEAM_SIDEBAR_SCOPE_SWITCH_CLASS,
  TEAM_SIDEBAR_SCOPE_BUTTON_ACTIVE_CLASS,
  TEAM_SIDEBAR_SCOPE_BUTTON_IDLE_CLASS,
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
  developerMode: boolean;
  busy: string | null;
  onRefreshTeams: () => Promise<void> | void;
  onOpenCreateTeamWizard: () => void;
  onOpenCreateTeamManual: () => void;
  draftTeamName: string;
  leaderMemberId: string;
  configuredWorkerCount: number;
  teams: TeamDefinitionRecord[];
  selectedTeam: TeamDefinitionRecord | null;
  selectedTeamId: string | null;
  teamMemberSummaryByTeamId: Map<string, TeamMemberSummary>;
  memberLiveStates: TeamMemberLiveState[];
  selectedMemberId: string;
  tab: TeamTab;
  onSelectTeam: (teamId: string) => void;
  onSelectConversation: () => void;
  onSelectAgentTab: (memberId: string, tab: TeamTab) => void;
  onSelectUtilityTab: (tab: TeamTab) => void;
};

const TEAM_UTILITY_ITEMS: ReadonlyArray<{ value: TeamTab; label: string }> = [
  { value: "runs", label: "Runs" },
];

const AGENT_FOCUS_TABS = new Set<TeamTab>(["agent_acp", "member_console", "mailbox"]);
const OPERATIONS_FOCUS_TABS = new Set<TeamTab>(["runs", "overview", "events", "steps", "debug"]);

type TeamSidebarScope = "subjects" | "operations";
type TeamSidebarSection = "channels" | "agents" | "utilities";

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

function resolveSidebarScope(tab: TeamTab): TeamSidebarScope {
  return OPERATIONS_FOCUS_TABS.has(tab) ? "operations" : "subjects";
}

const teamPickerItemBaseClass =
  "team-item flex w-full min-w-0 flex-col items-start gap-0.5 rounded-md border border-transparent px-2 py-2 text-left transition";
const teamPickerItemActiveClass =
  `${teamPickerItemBaseClass} bg-ui-surface-soft text-ui-text-primary`;
const teamPickerItemIdleClass =
  `${teamPickerItemBaseClass} text-ui-text-secondary hover:bg-ui-surface-soft/80 hover:text-ui-text-primary`;

export function TeamSidebar(props: TeamSidebarProps) {
  const {
    developerMode,
    busy,
    onRefreshTeams,
    onOpenCreateTeamWizard,
    onOpenCreateTeamManual,
    draftTeamName,
    leaderMemberId,
    configuredWorkerCount,
    teams,
    selectedTeam,
    selectedTeamId,
    teamMemberSummaryByTeamId,
    memberLiveStates,
    selectedMemberId,
    tab,
    onSelectTeam,
    onSelectConversation,
    onSelectAgentTab,
    onSelectUtilityTab,
  } = props;
  const [teamFilter, setTeamFilter] = React.useState("");
  const [teamPickerOpen, setTeamPickerOpen] = React.useState(selectedTeamId == null);
  const [teamDetailsOpen, setTeamDetailsOpen] = React.useState(false);
  const [teamActionsOpen, setTeamActionsOpen] = React.useState(false);
  const [sidebarScope, setSidebarScope] = React.useState<TeamSidebarScope>(
    resolveSidebarScope(tab)
  );
  const [sectionOpen, setSectionOpen] = React.useState<Record<TeamSidebarSection, boolean>>({
    channels: true,
    agents: true,
    utilities: true,
  });
  const normalizedTeamFilter = teamFilter.trim().toLowerCase();
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

  React.useEffect(() => {
    if (!selectedTeamId) {
      setTeamPickerOpen(true);
    }
  }, [selectedTeamId]);

  React.useEffect(() => {
    setSidebarScope(resolveSidebarScope(tab));
  }, [tab]);

  const handleSelectSubjectsScope = React.useCallback(() => {
    setSidebarScope("subjects");
    if (resolveSidebarScope(tab) === "operations") {
      if (selectedMemberId) {
        onSelectAgentTab(selectedMemberId, "agent_acp");
        return;
      }
      onSelectConversation();
    }
  }, [onSelectAgentTab, onSelectConversation, selectedMemberId, tab]);

  const handleSelectOperationsScope = React.useCallback(() => {
    setSidebarScope("operations");
    if (resolveSidebarScope(tab) !== "operations") {
      onSelectUtilityTab("runs");
    }
  }, [onSelectUtilityTab, tab]);

  const toggleSection = React.useCallback((section: TeamSidebarSection) => {
    setSectionOpen((current) => ({
      ...current,
      [section]: !current[section],
    }));
  }, []);

  return (
    <aside className={TEAM_SIDEBAR_ROOT_CLASS}>
      <div className="flex items-start gap-2">
        <button
          type="button"
          className={`${TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS} flex-1`}
          onClick={() => setTeamPickerOpen((current) => !current)}
          aria-expanded={teamPickerOpen}
          aria-label={`Toggle team switcher${selectedTeam ? `: ${selectedTeam.name}` : ""}`}
          title={
            selectedTeam
              ? developerMode
                ? selectedTeam.id
                : selectedTeam.name
              : "Toggle team switcher"
          }
        >
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold text-ui-text-primary">
              {selectedTeam?.name ?? "Select team"}
            </div>
            {!selectedTeam && (
              <div className="mono mt-0.5 truncate text-[11px] text-ui-text-muted">
                {`${teams.length} teams loaded`}
              </div>
            )}
          </div>
          <i
            className={`${teamPickerOpen ? "bi bi-chevron-up" : "bi bi-chevron-down"} text-ui-text-muted`}
            aria-hidden="true"
          />
        </button>
        <div className="flex items-center gap-2 pt-0.5">
          <button
            onClick={() => {
              void onRefreshTeams();
            }}
            disabled={busy === "refresh-teams"}
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            title="Refresh teams"
            aria-label="Refresh teams"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </button>
          <div className="relative">
              <button
                type="button"
                className={TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS}
                aria-label="Open team actions"
                title="Open team actions"
                aria-expanded={teamActionsOpen}
                onClick={() => setTeamActionsOpen((current) => !current)}
              >
                <i className="bi bi-three-dots" aria-hidden="true" />
              </button>
            {teamActionsOpen && (
              <div className="absolute right-0 top-full z-20 mt-2 flex min-w-44 flex-col gap-1 rounded-lg border border-ui-border bg-ui-surface p-2 shadow-lg">
                <button
                  type="button"
                  className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} w-full justify-start`}
                  onClick={() => {
                    setTeamActionsOpen(false);
                    onOpenCreateTeamWizard();
                  }}
                >
                  Guided Wizard
                </button>
                <button
                  type="button"
                  className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} w-full justify-start`}
                  onClick={() => {
                    setTeamActionsOpen(false);
                    onOpenCreateTeamManual();
                  }}
                >
                  Manual Spec
                </button>
                {developerMode && (
                  <>
                    <div className="my-1 border-t border-ui-border/80" />
                    <button
                      type="button"
                      className={`${TEAM_PANEL_GHOST_BUTTON_CLASS} w-full justify-start`}
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
      </div>

      {teamPickerOpen && (
        <div className={TEAM_SIDEBAR_SWITCHER_PANEL_CLASS}>
          {teams.length > 0 && (
            <div className="teams-filter flex items-center gap-2">
              <input
                className={TEAM_PANEL_INPUT_CLASS}
                placeholder="Filter teams by name or id"
                aria-label="Filter teams"
                value={teamFilter}
                onChange={(event) => setTeamFilter(event.target.value)}
              />
              {hasTeamFilter && (
                <button
                  type="button"
                  className={TEAM_PANEL_GHOST_BUTTON_CLASS}
                  onClick={() => setTeamFilter("")}
                  aria-label="Clear team filter"
                  title="Clear team filter"
                >
                  Clear
                </button>
              )}
            </div>
          )}

          <div className="teams-list mt-3 flex max-h-64 min-h-0 flex-col gap-2 overflow-auto">
            {teams.length === 0 && <p className={TEAM_MUTED_TEXT_CLASS}>No teams yet.</p>}
            {teams.length > 0 && filteredTeams.length === 0 && (
              <p className={TEAM_MUTED_TEXT_CLASS}>No teams match current filter.</p>
            )}
            {hasTeamFilter && filteredTeams.length > 0 && (
              <p className={`${TEAM_MUTED_TEXT_CLASS} mono`}>{`filtered=${filteredTeams.length} total=${teams.length}`}</p>
            )}
            {filteredTeams.map((team) => {
              const summary = teamMemberSummaryByTeamId.get(team.id);
              return (
                <button
                  key={team.id}
                  type="button"
                  className={
                    team.id === selectedTeamId
                      ? teamPickerItemActiveClass
                      : teamPickerItemIdleClass
                  }
                  onClick={() => {
                    onSelectTeam(team.id);
                  }}
                  aria-current={team.id === selectedTeamId ? "true" : undefined}
                  data-team-selected={team.id === selectedTeamId ? "true" : "false"}
                  title={developerMode ? team.id : team.name}
                >
                  <span className={TEAM_LIST_ITEM_TITLE_CLASS}>{team.name}</span>
                  {developerMode && (
                    <span className={`${TEAM_LIST_ITEM_META_CLASS} opacity-80`}>{team.id}</span>
                  )}
                  {summary && (
                    <span className={`${TEAM_LIST_ITEM_META_CLASS} opacity-70`}>
                      {`active=${summary.active} inactive=${summary.inactive} missing=${summary.missing} total=${summary.total}`}
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
                className={TEAM_PANEL_GHOST_BUTTON_CLASS}
                onClick={onOpenCreateTeamWizard}
              >
                Guided Wizard
              </button>
              <button
                type="button"
                className={TEAM_PANEL_GHOST_BUTTON_CLASS}
                onClick={onOpenCreateTeamManual}
              >
                Manual Spec
              </button>
            </div>
          )}

          {teamDetailsOpen && (
            <div className={`${TEAM_SIDEBAR_META_GRID_CLASS} border-t border-ui-border/70 pt-3`}>
              <span>draft_team={draftTeamName.trim() || "-"}</span>
              <span>leader={leaderMemberId.trim() || "-"}</span>
              <span>workers={configuredWorkerCount}</span>
            </div>
          )}
        </div>
      )}

      {selectedTeam && (
        <>
          <div className={TEAM_SIDEBAR_SCOPE_SWITCH_CLASS} aria-label="Team sidebar scope">
            <span className="shrink-0 px-1 text-[10px] font-semibold uppercase tracking-[0.22em] text-ui-text-muted">
              Index
            </span>
            <button
              type="button"
              className={
                sidebarScope === "subjects"
                  ? TEAM_SIDEBAR_SCOPE_BUTTON_ACTIVE_CLASS
                  : TEAM_SIDEBAR_SCOPE_BUTTON_IDLE_CLASS
              }
              onClick={handleSelectSubjectsScope}
            >
              Channels & Agents
            </button>
            <button
              type="button"
              className={
                sidebarScope === "operations"
                  ? TEAM_SIDEBAR_SCOPE_BUTTON_ACTIVE_CLASS
                  : TEAM_SIDEBAR_SCOPE_BUTTON_IDLE_CLASS
              }
              onClick={handleSelectOperationsScope}
            >
              Operations
            </button>
          </div>

          {sidebarScope === "subjects" && (
            <>
              <section className={TEAM_SIDEBAR_SECTION_CLASS}>
                <button
                  type="button"
                  className={TEAM_SIDEBAR_SECTION_TOGGLE_CLASS}
                  onClick={() => toggleSection("channels")}
                  aria-expanded={sectionOpen.channels}
                  aria-label="Toggle channels section"
                >
                  <span>Channels</span>
                  <i
                    className={
                      sectionOpen.channels ? "bi bi-chevron-down" : "bi bi-chevron-right"
                    }
                    aria-hidden="true"
                  />
                </button>
                {sectionOpen.channels && (
                  <div className={TEAM_SIDEBAR_NAV_LIST_CLASS}>
                    <button
                      type="button"
                      className={
                        tab === "conversation"
                          ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                          : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                      }
                      onClick={onSelectConversation}
                    >
                      <span className="text-sm font-semibold text-ui-text-primary">all</span>
                      <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
                        Shared team thread
                      </span>
                    </button>
                  </div>
                )}
              </section>

              <section className={TEAM_SIDEBAR_SECTION_CLASS}>
                <button
                  type="button"
                  className={TEAM_SIDEBAR_SECTION_TOGGLE_CLASS}
                  onClick={() => toggleSection("agents")}
                  aria-expanded={sectionOpen.agents}
                  aria-label="Toggle agents section"
                >
                  <span>{`Agents ${memberLiveStates.length}`}</span>
                  <i
                    className={
                      sectionOpen.agents ? "bi bi-chevron-down" : "bi bi-chevron-right"
                    }
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
                        selectedMemberId === member.member_id && AGENT_FOCUS_TABS.has(tab);
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
                              ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                              : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                          }
                          onClick={() => onSelectAgentTab(member.member_id, "agent_acp")}
                          title={
                            [primaryLabel, developerMode ? member.member_id : null, member.current_work]
                              .filter((value) => value && value.trim().length > 0)
                              .join(" · ") || member.member_id
                          }
                        >
                          <span className="flex w-full items-start justify-between gap-2">
                            <span className="truncate text-sm font-semibold text-ui-text-primary">
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
                                <span className="shrink-0 rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-[10px] font-semibold text-ui-text-secondary">
                                  {member.pending_inbox_count}
                                </span>
                              )}
                            </span>
                          </span>
                          {developerMode && memberMeta && (
                            <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>{memberMeta}</span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </section>
            </>
          )}

          {sidebarScope === "operations" && (
            <section className={TEAM_SIDEBAR_SECTION_CLASS}>
              <button
                type="button"
                className={TEAM_SIDEBAR_SECTION_TOGGLE_CLASS}
                onClick={() => toggleSection("utilities")}
                aria-expanded={sectionOpen.utilities}
                aria-label="Toggle utilities section"
              >
                <span>{`Utilities ${TEAM_UTILITY_ITEMS.length}`}</span>
                <i
                  className={
                    sectionOpen.utilities ? "bi bi-chevron-down" : "bi bi-chevron-right"
                  }
                  aria-hidden="true"
                />
              </button>
              {sectionOpen.utilities && (
                <div className={TEAM_SIDEBAR_NAV_LIST_CLASS}>
                  {TEAM_UTILITY_ITEMS.map((item) => (
                    <button
                      key={item.value}
                      type="button"
                      className={
                        tab === item.value
                          ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                          : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                      }
                      onClick={() => onSelectUtilityTab(item.value)}
                    >
                      <span className="text-sm font-semibold text-ui-text-primary">
                        {item.label}
                      </span>
                      <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
                        Browse runs
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </section>
          )}
        </>
      )}
    </aside>
  );
}
