import React from "react";
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
  TEAM_SIDEBAR_INFO_CARD_CLASS,
  TEAM_SIDEBAR_INFO_LABEL_CLASS,
  TEAM_SIDEBAR_INFO_TEXT_CLASS,
  TEAM_SIDEBAR_META_GRID_CLASS,
  TEAM_SIDEBAR_ROOT_CLASS,
} from "../ui/tailwind_classes";

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
  selectedTeamId: string | null;
  teamMemberSummaryByTeamId: Map<string, TeamMemberSummary>;
  onSelectTeam: (teamId: string) => void;
};

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
    selectedTeamId,
    teamMemberSummaryByTeamId,
    onSelectTeam,
  } = props;
  const [teamFilter, setTeamFilter] = React.useState("");
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

  return (
    <aside className={TEAM_SIDEBAR_ROOT_CLASS}>
      <div className="mode-switch mb-3 flex items-center gap-2">
        <a className="mode-tag" href="/">
          Agents
        </a>
        <a className="mode-tag active" href="/teams">
          Teams
        </a>
      </div>
      <div className="mb-3 flex items-center justify-between gap-2">
        <h2>Teams</h2>
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

      <div className={TEAM_SIDEBAR_FORGE_CARD_CLASS}>
        <h3 className="text-base font-semibold text-ui-text-primary">Team Forge</h3>
        <p className={TEAM_MUTED_TEXT_CLASS}>Choose a creation entry: guided wizard or direct manual spec.</p>
        <div className={TEAM_SIDEBAR_INFO_CARD_CLASS}>
          <p className={TEAM_SIDEBAR_INFO_LABEL_CLASS}>
            Operating Model
          </p>
          <p className={TEAM_SIDEBAR_INFO_TEXT_CLASS}>
            Leader plans and talks to human actor. Workers execute delegated tasks and report
            evidence back to leader.
          </p>
        </div>
        <div className="teams-create-entry-actions mt-3 flex flex-wrap gap-2">
          <button
            onClick={onOpenCreateTeamWizard}
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
          >
            Guided Wizard
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

      <div className="teams-list flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
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
              onClick={() => onSelectTeam(team.id)}
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
    </aside>
  );
}
