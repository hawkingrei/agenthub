import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
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
  "rounded-md border border-[color:var(--status-danger-border)] bg-[color:var(--status-danger-bg)] px-2 py-1 text-sm text-[color:var(--status-danger-ink)] transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_LIST_CLASS =
  "teams-run-list flex flex-col gap-2 rounded-xl border border-ui-border bg-ui-surface-soft/60 p-4";
const RUN_PANEL_LIST_HEAD_CLASS =
  "teams-run-list-head mb-2 flex flex-wrap items-center justify-between gap-2";
const RUN_PANEL_LIST_ITEMS_CLASS = "teams-run-list-items flex max-h-80 flex-col gap-2 overflow-y-auto pr-1";
const RUN_PANEL_SUBTITLE_CLASS = "mb-2 text-xs font-medium uppercase tracking-wide text-ui-text-muted";
const RUN_PANEL_HINT_TEXT_CLASS = "text-sm text-ui-text-muted";
const RUN_PANEL_LIST_TITLE_CLASS = "text-sm font-semibold text-ui-text-primary";
const RUN_PANEL_FOOT_META_CLASS = "mono text-ui-xs text-ui-text-muted";

type TeamRunPanelProps = {
  selectedTeam: TeamDefinitionRecord;
  developerMode: boolean;
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
    developerMode,
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
        <div className="text-lg font-semibold tracking-tight text-ui-text-primary">
          {selectedTeam.name}
        </div>
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
        <p className={TEAM_MUTED_TEXT_CLASS}>
          {developerMode ? (
            <>
              Team execution is agent-driven. You can quick-start here, or use{" "}
              <code>Debug → Run Ops</code> for manual run debugging.
            </>
          ) : (
            "Team execution is agent-driven. You can quick-start here. Manual run debugging is available in Developer Mode."
          )}
        </p>
        <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
          <span className={RUN_PANEL_HINT_TEXT_CLASS}>Quick start a new run for this team.</span>
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
          <h3 className={RUN_PANEL_LIST_TITLE_CLASS}>Runs</h3>
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
            <p className={TEAM_MUTED_TEXT_CLASS}>
              {developerMode
                ? "No runs loaded yet. Use Debug → Run Ops to create or load runs."
                : "No runs loaded yet. Enable Developer Mode for manual run debugging tools."}
            </p>
          )}
          {isActiveRunHiddenByFilter && activeRun && (
            <p className={TEAM_MUTED_TEXT_CLASS}>
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
          <span className={RUN_PANEL_FOOT_META_CLASS}>
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
