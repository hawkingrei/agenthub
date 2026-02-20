import React from "react";
import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_ICON_BUTTON_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

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

const RUN_PANEL_DELETE_BUTTON_CLASS =
  "rounded-md border border-rose-200 bg-rose-50 px-2 py-1 text-sm text-rose-700 hover:border-rose-300 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_MEMBER_CLASS =
  "teams-run-create rounded-xl border border-slate-200 bg-slate-50/60 p-4";
const RUN_PANEL_GRID_CLASS = "grid gap-3 xl:grid-cols-2";
const RUN_PANEL_SECTION_CLASS =
  "teams-run-create rounded-xl border border-slate-200 bg-slate-50/70 p-4";
const RUN_PANEL_LIST_CLASS =
  "teams-run-list rounded-xl border border-slate-200 bg-slate-50/50 p-4";
const RUN_PANEL_LIST_HEAD_CLASS =
  "teams-run-list-head mb-2 flex flex-wrap items-center justify-between gap-2";
const RUN_PANEL_LIST_ITEMS_CLASS = "teams-run-list-items flex max-h-80 flex-col gap-2 overflow-y-auto pr-1";
const RUN_PANEL_SUBTITLE_CLASS = "mb-2 text-xs font-medium uppercase tracking-wide text-slate-500";
const RUN_PANEL_MEMBER_SUMMARY_CLASS =
  "teams-member-summary-line mono mb-2 text-left text-xs tracking-wide text-slate-600";

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
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h2>{selectedTeam.name}</h2>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
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
      <div className={RUN_PANEL_MEMBER_CLASS}>
        <p className={RUN_PANEL_SUBTITLE_CLASS}>Team Members</p>
        <div className="teams-member-status-panel">
          <div className={RUN_PANEL_MEMBER_SUMMARY_CLASS}>
            {`team_number=${selectedTeamMemberSummary.total}`}
          </div>
          {selectedTeamMemberLiveStates.length === 0 ? (
            <p className="muted">No members declared in team spec.</p>
          ) : (
            <div className="teams-member-strip compact">
              {selectedTeamMemberLiveStates.map((member) => {
                const isRunning = member.lifecycle_tone === "active";
                return (
                  <span
                    key={`${selectedTeam.id}:${member.member_id}`}
                    className="teams-member-dot-item mono"
                    title={`member=${member.member_id} status=${member.lifecycle_status}`}
                  >
                    <span
                      aria-hidden="true"
                      className={`teams-member-dot ${isRunning ? "active" : "inactive"}`}
                    />
                    <span className="teams-member-dot-label">{member.member_id}</span>
                  </span>
                );
              })}
            </div>
          )}
        </div>
      </div>
      <div className={RUN_PANEL_GRID_CLASS}>
        <div className={RUN_PANEL_SECTION_CLASS}>
          <p className={RUN_PANEL_SUBTITLE_CLASS}>Run Controls</p>
          <h3>Create / Load Run</h3>
          <p className="muted">
            <strong>Create Run</strong> starts a new execution for this team spec.
            <br />
            <strong>Load Run</strong> opens an existing run by `run_id` (even if it was created earlier) and auto-switches to its team.
          </p>
          <div className="form-row">
            <input
              className={TEAM_PANEL_INPUT_CLASS}
              placeholder="context_id (optional, auto-generated when empty)"
              value={runContextId}
              onChange={(event) => onRunContextIdChange(event.target.value)}
            />
            <button
              className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
              onClick={onCreateRun}
              disabled={busy === "create-run"}
            >
              Create Run
            </button>
          </div>
          <textarea
            className={TEAM_PANEL_TEXTAREA_CLASS}
            rows={4}
            value={runInput}
            onChange={(event) => onRunInputChange(event.target.value)}
          />
          <div className="form-row">
            <input
              className={TEAM_PANEL_INPUT_CLASS}
              placeholder="existing run_id"
              value={runLookupId}
              onChange={(event) => onRunLookupIdChange(event.target.value)}
            />
            <button
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
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
            <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
              <select
                className={TEAM_PANEL_INPUT_CLASS}
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
                className={TEAM_PANEL_ICON_BUTTON_CLASS}
                title="Refresh runs"
                aria-label="Refresh runs"
              >
                <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              </button>
            </div>
          </div>
          <div className={RUN_PANEL_LIST_ITEMS_CLASS}>
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
          </div>
          <div className="teams-run-list-foot">
            <span className="mono">
              showing={visibleRuns.length} loaded={totalLoadedRunsForTeam} limit={pageLimit}
            </span>
            <button
              onClick={() => {
                void onLoadMoreRuns();
              }}
              disabled={runsLoading || !runsHasMore || !selectedTeamId}
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            >
              {runsLoading ? "Loading..." : runsHasMore ? "Load More" : "No More Runs"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
