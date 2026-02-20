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

  return (
    <aside className="card teams-sidebar rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur">
      <div className="mode-switch mb-3 flex items-center gap-2">
        <a className="mode-tag" href="/">
          Agents
        </a>
        <a className="mode-tag active" href="/teams">
          Teams
        </a>
      </div>
      <div className="toolbar mb-3 flex items-center justify-between gap-2">
        <h2>Teams</h2>
        <button
          onClick={() => {
            void onRefreshTeams();
          }}
          disabled={busy === "refresh-teams"}
          className="rounded-md border border-slate-300 bg-white px-2 py-1 text-sm hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          Refresh
        </button>
      </div>

      <div className="teams-form teams-create-launch rounded-xl border border-slate-200 bg-slate-50/70 p-4">
        <h3 className="text-base font-semibold text-slate-900">Team Forge</h3>
        <p className="muted">Choose a creation entry: guided wizard or direct manual spec.</p>
        <div className="teams-create-entry-actions mt-3 flex flex-wrap gap-2">
          <button
            onClick={onOpenCreateTeamWizard}
            className="rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800"
          >
            Guided Wizard
          </button>
          <button
            className="ghost rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium hover:border-slate-400"
            onClick={onOpenCreateTeamManual}
          >
            Manual Spec
          </button>
        </div>
        <div className="teams-create-launch-meta mono mt-3 grid gap-1 text-xs text-slate-600">
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
