import React from "react";
import { TeamDefinitionRecord } from "../api";
import {
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
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

  return (
    <aside className="teams-sidebar flex min-h-0 min-w-0 flex-col gap-3 rounded-2xl border border-slate-200 bg-white shadow-sm">
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

      <div className="teams-form teams-create-launch flex flex-col gap-2 rounded-xl border border-slate-200 bg-slate-50/70 p-4">
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

      <div className="teams-list flex max-h-80 flex-col gap-2 overflow-auto">
        {teams.length === 0 && <p className="muted">No teams yet.</p>}
        {teams.map((team) => {
          const summary = teamMemberSummaryByTeamId.get(team.id);
          return (
            <button
              key={team.id}
              className={
                team.id === selectedTeamId
                  ? TEAM_LIST_ITEM_ACTIVE_CLASS
                  : TEAM_LIST_ITEM_IDLE_CLASS
              }
              onClick={() => onSelectTeam(team.id)}
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
