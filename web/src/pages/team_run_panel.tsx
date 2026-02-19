import React from "react";
import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import {
  StatusBadge,
  resolveTeamLifecycleStatusTone,
  resolveTeamRunStatusTone,
} from "../components/status_badge";

type TeamRunStatusFilter = TeamRunStatus | "all";

type TeamRunStatusFilterOption = {
  value: TeamRunStatusFilter;
  label: string;
};

type TeamMemberSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

type TeamMemberLiveState = {
  member_id: string;
  role: string;
  agent_name?: string;
  lifecycle_status: string;
  lifecycle_tone: "active" | "inactive" | "missing";
  run_status: string;
  step_status: string;
  pending_inbox_count: number | null;
  current_work: string;
};

type TeamRunPanelProps = {
  selectedTeam: TeamDefinitionRecord;
  busy: string | null;
  onDeleteTeam: () => Promise<void> | void;
  selectedTeamMemberSummary: TeamMemberSummary;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  runContextId: string;
  onRunContextIdChange: (value: string) => void;
  onCreateRun: () => Promise<void> | void;
  runInput: string;
  onRunInputChange: (value: string) => void;
  runLookupId: string;
  onRunLookupIdChange: (value: string) => void;
  onLoadRunById: () => Promise<void> | void;
  runStatusFilter: TeamRunStatusFilter;
  runStatusFilterOptions: TeamRunStatusFilterOption[];
  onRunStatusFilterChange: (value: TeamRunStatusFilter) => void;
  onRefreshRuns: () => Promise<void> | void;
  runsLoading: boolean;
  visibleRuns: TeamRunRecord[];
  activeRunId: string | null;
  onActiveRunChange: (runId: string) => void;
  isActiveRunHiddenByFilter: boolean;
  activeRun: TeamRunRecord | null;
  totalLoadedRunsForTeam: number;
  pageLimit: number;
  runsHasMore: boolean;
  selectedTeamId: string | null;
  onLoadMoreRuns: () => Promise<void> | void;
};

export function TeamRunPanel(props: TeamRunPanelProps) {
  const {
    selectedTeam,
    busy,
    onDeleteTeam,
    selectedTeamMemberSummary,
    selectedTeamMemberLiveStates,
    runContextId,
    onRunContextIdChange,
    onCreateRun,
    runInput,
    onRunInputChange,
    runLookupId,
    onRunLookupIdChange,
    onLoadRunById,
    runStatusFilter,
    runStatusFilterOptions,
    onRunStatusFilterChange,
    onRefreshRuns,
    runsLoading,
    visibleRuns,
    activeRunId,
    onActiveRunChange,
    isActiveRunHiddenByFilter,
    activeRun,
    totalLoadedRunsForTeam,
    pageLimit,
    runsHasMore,
    selectedTeamId,
    onLoadMoreRuns,
  } = props;

  return (
    <div className="card">
      <div className="toolbar">
        <h2>{selectedTeam.name}</h2>
        <div className="actions">
          <span className="mono">{selectedTeam.id}</span>
          <button
            onClick={() => {
              void onDeleteTeam();
            }}
            disabled={busy === "delete-team"}
          >
            Delete Team
          </button>
        </div>
      </div>
      <div className="teams-run-create">
        <div className="teams-member-status-panel">
          <div className="teams-member-summary-line mono">
            {`active=${selectedTeamMemberSummary.active} inactive=${selectedTeamMemberSummary.inactive} missing=${selectedTeamMemberSummary.missing} total=${selectedTeamMemberSummary.total}`}
          </div>
          {selectedTeamMemberLiveStates.length === 0 ? (
            <p className="muted">No members declared in team spec.</p>
          ) : (
            <div className="teams-member-strip">
              {selectedTeamMemberLiveStates.map((member) => {
                const roleTone = member.role.trim().toLowerCase() === "leader" ? "leader" : "worker";
                return (
                  <article key={`${selectedTeam.id}:${member.member_id}`} className="team-member-row">
                    <div className="team-member-row-main">
                      <span className={`team-role-chip ${roleTone}`}>{member.role}</span>
                      <span className="team-member-row-id mono">{member.member_id}</span>
                      <StatusBadge
                        label={member.lifecycle_status}
                        tone={resolveTeamLifecycleStatusTone(member.lifecycle_tone)}
                        className={`team-status-chip ${member.lifecycle_tone}`}
                        title={`lifecycle: ${member.lifecycle_status}`}
                      />
                      <span className="team-member-row-meta mono">
                        {member.agent_name ?? "agent_not_found"}
                      </span>
                      <span className="team-member-row-meta mono">
                        {`run=${member.run_status} step=${member.step_status} inbox=${member.pending_inbox_count ?? "-"}`}
                      </span>
                    </div>
                    <div className="team-member-workline mono">{member.current_work}</div>
                  </article>
                );
              })}
            </div>
          )}
        </div>
        <h3>Create / Load Run</h3>
        <p className="muted">
          <strong>Create Run</strong> starts a new execution for this team spec.
          <br />
          <strong>Load Run</strong> opens an existing run by `run_id` (even if it was created earlier) and auto-switches to its team.
        </p>
        <div className="form-row">
          <input
            placeholder="context_id (optional, auto-generated when empty)"
            value={runContextId}
            onChange={(event) => onRunContextIdChange(event.target.value)}
          />
          <button onClick={onCreateRun} disabled={busy === "create-run"}>
            Create Run
          </button>
        </div>
        <textarea
          className="mono"
          rows={4}
          value={runInput}
          onChange={(event) => onRunInputChange(event.target.value)}
        />
        <div className="form-row">
          <input
            placeholder="existing run_id"
            value={runLookupId}
            onChange={(event) => onRunLookupIdChange(event.target.value)}
          />
          <button onClick={onLoadRunById} disabled={busy === "load-run"}>
            Load Run
          </button>
        </div>
      </div>
      <div className="teams-run-list">
        <div className="teams-run-list-head">
          <h3>Runs</h3>
          <div className="actions">
            <select
              value={runStatusFilter}
              onChange={(event) => onRunStatusFilterChange(event.target.value as TeamRunStatusFilter)}
              aria-label="Run status filter"
            >
              {runStatusFilterOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => {
                void onRefreshRuns();
              }}
              disabled={runsLoading}
            >
              Refresh Runs
            </button>
          </div>
        </div>
        {visibleRuns.length === 0 && (
          <p className="muted">No runs loaded yet. Create one or load by run_id.</p>
        )}
        {isActiveRunHiddenByFilter && activeRun && (
          <p className="muted">
            Active run `{activeRun.id}` is hidden by filter `{runStatusFilter}`.
          </p>
        )}
        {visibleRuns.map((run) => (
          <button
            key={run.id}
            className={run.id === activeRunId ? "team-item active" : "team-item"}
            onClick={() => onActiveRunChange(run.id)}
          >
            <span className="team-name mono">{run.id}</span>
            <StatusBadge
              label={run.status}
              tone={resolveTeamRunStatusTone(run.status)}
              className="team-status"
              title={`run status: ${run.status}`}
            />
          </button>
        ))}
        <div className="teams-run-list-foot">
          <span className="mono">
            showing={visibleRuns.length} loaded={totalLoadedRunsForTeam} limit={pageLimit}
          </span>
          <button
            onClick={() => {
              void onLoadMoreRuns();
            }}
            disabled={runsLoading || !runsHasMore || !selectedTeamId}
          >
            {runsLoading ? "Loading..." : runsHasMore ? "Load More" : "No More Runs"}
          </button>
        </div>
      </div>
    </div>
  );
}
