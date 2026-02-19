import React from "react";
import { TeamDefinitionRecord } from "../api";

type TeamMemberSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

type TeamSidebarProps = {
  busy: string | null;
  onRefreshTeams: () => Promise<void> | void;
  onOpenCreateTeamModal: () => void;
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
    onOpenCreateTeamModal,
    draftTeamName,
    leaderMemberId,
    configuredWorkerCount,
    teams,
    selectedTeamId,
    teamMemberSummaryByTeamId,
    onSelectTeam,
  } = props;

  return (
    <aside className="card teams-sidebar">
      <div className="mode-switch">
        <a className="mode-tag" href="/">
          Agents
        </a>
        <a className="mode-tag active" href="/teams">
          Teams
        </a>
      </div>
      <div className="toolbar">
        <h2>Teams</h2>
        <button
          onClick={() => {
            void onRefreshTeams();
          }}
          disabled={busy === "refresh-teams"}
        >
          Refresh
        </button>
      </div>

      <div className="teams-form teams-create-launch">
        <h3>Team Forge</h3>
        <p className="muted">Open the creation quest to set up Leader and Workers in stages.</p>
        <button onClick={onOpenCreateTeamModal}>Create Team</button>
        <div className="teams-create-launch-meta mono">
          <span>draft_team={draftTeamName.trim() || "-"}</span>
          <span>leader={leaderMemberId.trim() || "-"}</span>
          <span>workers={configuredWorkerCount}</span>
        </div>
      </div>

      <div className="teams-list">
        {teams.length === 0 && <p className="muted">No teams yet.</p>}
        {teams.map((team) => {
          const summary = teamMemberSummaryByTeamId.get(team.id);
          return (
            <button
              key={team.id}
              className={team.id === selectedTeamId ? "team-item active" : "team-item"}
              onClick={() => onSelectTeam(team.id)}
            >
              <span className="team-name">{team.name}</span>
              <span className="team-id mono">{team.id}</span>
              {summary && (
                <span className="team-id mono team-member-summary">
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
