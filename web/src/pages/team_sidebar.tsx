import React from "react";
import { Collapse } from "@mantine/core";
import { TeamDefinitionRecord } from "../api";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_GHOST_BUTTON_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_SIDEBAR_FORGE_CARD_CLASS,
  TEAM_SIDEBAR_META_GRID_CLASS,
  TEAM_SIDEBAR_ROOT_CLASS,
  TEAM_SIDEBAR_SECTION_CLASS,
  TEAM_SIDEBAR_SECTION_LABEL_CLASS,
  TEAM_SIDEBAR_NAV_LIST_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS,
  TEAM_SIDEBAR_NAV_ITEM_META_CLASS,
  TEAM_SIDEBAR_SUBNAV_CLASS,
  TEAM_SIDEBAR_SUBNAV_BUTTON_ACTIVE_CLASS,
  TEAM_SIDEBAR_SUBNAV_BUTTON_IDLE_CLASS,
  TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS,
  TEAM_SIDEBAR_SWITCHER_PANEL_CLASS,
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

const AGENT_VIEW_ITEMS: ReadonlyArray<{ value: TeamTab; label: string }> = [
  { value: "agent_acp", label: "ACP" },
  { value: "member_console", label: "Console" },
  { value: "mailbox", label: "Mailbox" },
];

const TEAM_UTILITY_ITEMS: ReadonlyArray<{ value: TeamTab; label: string }> = [
  { value: "runs", label: "Runs" },
  { value: "overview", label: "Overview" },
  { value: "events", label: "Events" },
  { value: "steps", label: "Steps" },
  { value: "debug", label: "Debug" },
];

const AGENT_FOCUS_TABS = new Set<TeamTab>(["agent_acp", "member_console", "mailbox"]);

export function TeamSidebar(props: TeamSidebarProps) {
  const {
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

  return (
    <aside className={TEAM_SIDEBAR_ROOT_CLASS}>
      <div className="flex items-center justify-between gap-2">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-ui-text-muted">
            Teams
          </p>
          <h2 className="mt-1 text-base font-semibold text-ui-text-primary">Workbench</h2>
        </div>
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
      </div>

      <button
        type="button"
        className={TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS}
        onClick={() => setTeamPickerOpen((current) => !current)}
        aria-expanded={teamPickerOpen}
        aria-label="Toggle team switcher"
      >
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-ui-text-primary">
            {selectedTeam?.name ?? "Select team"}
          </div>
          <div className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
            {selectedTeam ? selectedTeam.id : `${teams.length} teams loaded`}
          </div>
        </div>
        <i
          className={teamPickerOpen ? "bi bi-chevron-up" : "bi bi-chevron-down"}
          aria-hidden="true"
        />
      </button>

      <Collapse in={teamPickerOpen}>
        <div className={TEAM_SIDEBAR_SWITCHER_PANEL_CLASS}>
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
                      ? TEAM_LIST_ITEM_ACTIVE_CLASS
                      : TEAM_LIST_ITEM_IDLE_CLASS
                  }
                  onClick={() => {
                    onSelectTeam(team.id);
                    setTeamPickerOpen(false);
                  }}
                  aria-current={team.id === selectedTeamId ? "true" : undefined}
                  data-team-selected={team.id === selectedTeamId ? "true" : "false"}
                  title={team.id}
                >
                  <span className={TEAM_LIST_ITEM_TITLE_CLASS}>{team.name}</span>
                  <span className={TEAM_LIST_ITEM_META_CLASS}>{team.id}</span>
                  {summary && (
                    <span className={`${TEAM_LIST_ITEM_META_CLASS} opacity-80`}>
                      {`active=${summary.active} inactive=${summary.inactive} missing=${summary.missing} total=${summary.total}`}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </Collapse>

      <div className={TEAM_SIDEBAR_FORGE_CARD_CLASS}>
        <div className="flex flex-wrap gap-2">
          <button onClick={onOpenCreateTeamWizard} className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}>
            Create Team
          </button>
          <button
            className={TEAM_PANEL_GHOST_BUTTON_CLASS}
            onClick={onOpenCreateTeamManual}
          >
            Manual Spec
          </button>
        </div>
        <div className={TEAM_SIDEBAR_META_GRID_CLASS}>
          <span>draft_team={draftTeamName.trim() || "-"}</span>
          <span>leader={leaderMemberId.trim() || "-"}</span>
          <span>workers={configuredWorkerCount}</span>
        </div>
      </div>

      {selectedTeam && (
        <>
          <section className={TEAM_SIDEBAR_SECTION_CLASS}>
            <div className={TEAM_SIDEBAR_SECTION_LABEL_CLASS}>Human</div>
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
                <span className="text-sm font-semibold text-ui-text-primary">Conversation</span>
                <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
                  Shared planning and human coordination lane
                </span>
              </button>
            </div>
          </section>

          <section className={TEAM_SIDEBAR_SECTION_CLASS}>
            <div className={TEAM_SIDEBAR_SECTION_LABEL_CLASS}>Agents</div>
            <div className={TEAM_SIDEBAR_NAV_LIST_CLASS}>
              {memberLiveStates.length === 0 && (
                <p className={TEAM_MUTED_TEXT_CLASS}>No members found in current team spec.</p>
              )}
              {memberLiveStates.map((member) => {
                const lifecycle = normalizeTeamMemberLifecycle(member);
                const workStatus = normalizeTeamMemberWorkStatus(member);
                const isSelectedMember = selectedMemberId === member.member_id;
                const isActiveMember = isSelectedMember && AGENT_FOCUS_TABS.has(tab);
                return (
                  <div key={member.member_id} className="min-w-0">
                    <button
                      type="button"
                      className={
                        isActiveMember
                          ? TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS
                          : TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS
                      }
                      onClick={() => onSelectAgentTab(member.member_id, "agent_acp")}
                    >
                      <span className="truncate text-sm font-semibold text-ui-text-primary">
                        {member.member_id}
                      </span>
                      <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
                        {`role=${member.role} agent=${member.agent_name ?? "-"} lifecycle=${lifecycle} work=${workStatus}`}
                      </span>
                      <span
                        className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}
                        title={member.current_work}
                      >
                        {`current=${member.current_work}`}
                      </span>
                    </button>
                    {isSelectedMember && (
                      <div className={TEAM_SIDEBAR_SUBNAV_CLASS}>
                        {AGENT_VIEW_ITEMS.map((item) => (
                          <button
                            key={item.value}
                            type="button"
                            className={
                              tab === item.value
                                ? TEAM_SIDEBAR_SUBNAV_BUTTON_ACTIVE_CLASS
                                : TEAM_SIDEBAR_SUBNAV_BUTTON_IDLE_CLASS
                            }
                            onClick={() => onSelectAgentTab(member.member_id, item.value)}
                          >
                            {item.label}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </section>

          <section className={TEAM_SIDEBAR_SECTION_CLASS}>
            <div className={TEAM_SIDEBAR_SECTION_LABEL_CLASS}>Utilities</div>
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
                  <span className="text-sm font-semibold text-ui-text-primary">{item.label}</span>
                  <span className={TEAM_SIDEBAR_NAV_ITEM_META_CLASS}>
                    {item.value === "runs"
                      ? "Run browser and execution entry"
                      : item.value === "overview"
                        ? "Snapshot and member overview"
                        : item.value === "events"
                          ? "Run event timeline"
                          : item.value === "steps"
                            ? "Execution steps and actions"
                            : "Operational diagnostics"}
                  </span>
                </button>
              ))}
            </div>
          </section>
        </>
      )}
    </aside>
  );
}
