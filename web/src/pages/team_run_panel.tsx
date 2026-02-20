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

const RUN_PANEL_CARD_CLASS =
  "card rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur";
const RUN_PANEL_TOOLBAR_CLASS = "toolbar mb-3 flex items-center justify-between gap-2";
const RUN_PANEL_TOOLBAR_ACTIONS_CLASS = "actions flex items-center gap-2";
const RUN_PANEL_DELETE_BUTTON_CLASS =
  "rounded-md border border-rose-200 bg-rose-50 px-2 py-1 text-sm text-rose-700 hover:border-rose-300 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_SECTION_CLASS = "teams-run-create rounded-xl border border-slate-200 bg-slate-50/70 p-4";
const RUN_PANEL_INPUT_CLASS =
  "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";
const RUN_PANEL_TEXTAREA_CLASS =
  "mono min-h-24 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";
const RUN_PANEL_PRIMARY_BUTTON_CLASS =
  "rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_SECONDARY_BUTTON_CLASS =
  "rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-900 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_LIST_CLASS = "teams-run-list rounded-xl border border-slate-200 bg-slate-50/50 p-4";
const RUN_PANEL_LIST_HEAD_CLASS =
  "teams-run-list-head mb-2 flex flex-wrap items-center justify-between gap-2";
const RUN_PANEL_LIST_ACTIONS_CLASS = "actions flex items-center gap-2";

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
    <div className={RUN_PANEL_CARD_CLASS}>
      <div className={RUN_PANEL_TOOLBAR_CLASS}>
        <h2>{selectedTeam.name}</h2>
        <div className={RUN_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <span className="mono">{selectedTeam.id}</span>
          <button
            onClick={() => {
              void onDeleteTeam();
            }}
            disabled={busy === "delete-team"}
            className={RUN_PANEL_DELETE_BUTTON_CLASS}
          >
            Delete Team
          </button>
        </div>
      </div>
      <div className={RUN_PANEL_SECTION_CLASS}>
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
            className={RUN_PANEL_INPUT_CLASS}
            placeholder="context_id (optional, auto-generated when empty)"
            value={runContextId}
            onChange={(event) => onRunContextIdChange(event.target.value)}
          />
          <button
            className={RUN_PANEL_PRIMARY_BUTTON_CLASS}
            onClick={onCreateRun}
            disabled={busy === "create-run"}
          >
            Create Run
          </button>
        </div>
        <textarea
          className={RUN_PANEL_TEXTAREA_CLASS}
          rows={4}
          value={runInput}
          onChange={(event) => onRunInputChange(event.target.value)}
        />
        <div className="form-row">
          <input
            className={RUN_PANEL_INPUT_CLASS}
            placeholder="existing run_id"
            value={runLookupId}
            onChange={(event) => onRunLookupIdChange(event.target.value)}
          />
          <button
            className={RUN_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={onLoadRunById}
            disabled={busy === "load-run"}
          >
            Load Run
          </button>
        </div>
      </div>
      <div className={RUN_PANEL_LIST_CLASS}>
        <div className={RUN_PANEL_LIST_HEAD_CLASS}>
          <h3>Runs</h3>
          <div className={RUN_PANEL_LIST_ACTIONS_CLASS}>
            <select
              className={RUN_PANEL_INPUT_CLASS}
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
              className={RUN_PANEL_SECONDARY_BUTTON_CLASS}
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
            className={RUN_PANEL_SECONDARY_BUTTON_CLASS}
          >
            {runsLoading ? "Loading..." : runsHasMore ? "Load More" : "No More Runs"}
          </button>
        </div>
      </div>
    </div>
  );
}
