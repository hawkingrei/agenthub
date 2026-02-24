import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamRunStatusFilter = TeamRunStatus | "all";

type TeamRunStatusFilterOption = {
  value: TeamRunStatusFilter;
  label: string;
};

const RUN_PANEL_DELETE_BUTTON_CLASS =
  "rounded-md border border-rose-200 bg-rose-50 px-2 py-1 text-sm text-rose-700 hover:border-rose-300 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_LIST_CLASS =
  "teams-run-list flex flex-col gap-2 rounded-xl border border-slate-200 bg-slate-50/50 p-4";
const RUN_PANEL_LIST_HEAD_CLASS =
  "teams-run-list-head mb-2 flex flex-wrap items-center justify-between gap-2";
const RUN_PANEL_LIST_ITEMS_CLASS = "teams-run-list-items flex max-h-80 flex-col gap-2 overflow-y-auto pr-1";
const RUN_PANEL_SUBTITLE_CLASS = "mb-2 text-xs font-medium uppercase tracking-wide text-slate-500";

type TeamRunPanelProps = {
  selectedTeam: TeamDefinitionRecord;
  busy: string | null;
  onDeleteTeam: () => Promise<void> | void;
  onStartTeam: () => Promise<void> | void;
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
    onStartTeam,
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
      <div className={RUN_PANEL_LIST_CLASS}>
        <p className={RUN_PANEL_SUBTITLE_CLASS}>Run Browser</p>
        <p className="muted text-sm">
          Team execution is agent-driven. You can quick-start here, or use <code>Debug → Run Ops</code>{" "}
          for manual run debugging.
        </p>
        <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
          <span className="text-sm text-slate-600">Quick start a new run for this team.</span>
          <button
            onClick={() => {
              void onStartTeam();
            }}
            disabled={busy === "create-run" || !selectedTeamId}
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            title="Start a new run for the selected team"
            aria-label="Start team"
          >
            {busy === "create-run" ? "Starting..." : "Start Team"}
          </button>
        </div>
        <div className={RUN_PANEL_LIST_HEAD_CLASS}>
          <h3>Runs</h3>
          <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
            <select
              className={`${TEAM_PANEL_INPUT_CLASS} min-w-0 sm:min-w-[164px]`}
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
              className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
              title="Refresh runs"
              aria-label="Refresh runs"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              <span>Refresh</span>
            </button>
          </div>
        </div>
        <div className={RUN_PANEL_LIST_ITEMS_CLASS}>
          {visibleRuns.length === 0 && (
            <p className="muted">No runs loaded yet. Use Debug → Run Ops to create or load runs.</p>
          )}
          {isActiveRunHiddenByFilter && activeRun && (
            <p className="muted">
              Active run `{activeRun.id}` is hidden by filter `{runStatusFilter}`.
            </p>
          )}
          {visibleRuns.map((run) => (
            <button
              key={run.id}
              className={
                run.id === activeRunId
                  ? TEAM_LIST_ITEM_ACTIVE_CLASS
                  : TEAM_LIST_ITEM_IDLE_CLASS
              }
              onClick={() => onActiveRunChange(run.id)}
            >
              <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} mono`}>{run.id}</span>
              <StatusBadge
                label={run.status}
                tone={resolveTeamRunStatusTone(run.status)}
                className="team-status"
                title={`run status: ${run.status}`}
              />
            </button>
          ))}
        </div>
        <div className="teams-run-list-foot flex flex-wrap items-center justify-between gap-2">
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
  );
}
