import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
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

type RunInputValidation = {
  parsed: unknown | undefined;
  error: string | null;
};

const RUN_PANEL_DELETE_BUTTON_CLASS =
  "rounded-md border border-rose-200 bg-rose-50 px-2 py-1 text-sm text-rose-700 hover:border-rose-300 disabled:cursor-not-allowed disabled:opacity-60";
const RUN_PANEL_GRID_CLASS = "grid gap-3 xl:grid-cols-2";
const RUN_PANEL_SECTION_CLASS =
  "teams-run-create flex flex-col gap-2 rounded-xl border border-slate-200 bg-slate-50/70 p-4";
const RUN_PANEL_LIST_CLASS =
  "teams-run-list flex flex-col gap-2 rounded-xl border border-slate-200 bg-slate-50/50 p-4";
const RUN_PANEL_LIST_HEAD_CLASS =
  "teams-run-list-head mb-2 flex flex-wrap items-center justify-between gap-2";
const RUN_PANEL_LIST_ITEMS_CLASS = "teams-run-list-items flex max-h-80 flex-col gap-2 overflow-y-auto pr-1";
const RUN_PANEL_SUBTITLE_CLASS = "mb-2 text-xs font-medium uppercase tracking-wide text-slate-500";

function validateRunInputJson(raw: string): RunInputValidation {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { parsed: undefined, error: null };
  }
  try {
    return { parsed: JSON.parse(trimmed), error: null };
  } catch (err) {
    const message = err instanceof Error ? err.message : "unknown parse error";
    return { parsed: undefined, error: `Run input must be valid JSON (${message})` };
  }
}

type TeamRunPanelProps = {
  selectedTeam: TeamDefinitionRecord;
  busy: string | null;
  onDeleteTeam: () => Promise<void> | void;
  runContextId: string;
  onRunContextIdChange: (value: string) => void;
  onCreateRun: () => Promise<void> | void;
  runInput: string;
  onRunInputChange: (value: string) => void;
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
    runContextId,
    onRunContextIdChange,
    onCreateRun,
    runInput,
    onRunInputChange,
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
  const runInputValidation = validateRunInputJson(runInput);
  const runInputHasError = runInputValidation.error !== null;
  const canSubmitRun = busy !== "create-run" && !runInputHasError;

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
      <div className={RUN_PANEL_GRID_CLASS}>
        <div className={RUN_PANEL_SECTION_CLASS}>
          <p className={RUN_PANEL_SUBTITLE_CLASS}>Run Controls</p>
          <h3>Create Run</h3>
          <p className="muted">
            Start a new execution for this team spec.
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
              disabled={!canSubmitRun}
              title={runInputValidation.error ?? "Create run"}
            >
              Create Run
            </button>
          </div>
          <p className="text-xs text-slate-500">
            <code>context_id</code> can be empty. Use one when you want retries/resume grouped
            under the same context.
          </p>
          <textarea
            className={TEAM_PANEL_TEXTAREA_CLASS}
            rows={8}
            placeholder='Optional JSON input, e.g. {"task":"sync"}'
            aria-label="Run input JSON"
            spellCheck={false}
            value={runInput}
            onChange={(event) => onRunInputChange(event.target.value)}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canSubmitRun) {
                event.preventDefault();
                void onCreateRun();
              }
            }}
          />
          {runInputValidation.error ? (
            <p className="text-xs text-rose-600" role="alert">
              {runInputValidation.error}
            </p>
          ) : (
            <p className="text-xs text-slate-500">
              Accepts any valid JSON value. Shortcut: Ctrl/Cmd + Enter to create run.
            </p>
          )}
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              onClick={() =>
                onRunInputChange(
                  JSON.stringify(
                    {
                      task: "investigate",
                      objective: "improve-team-run",
                    },
                    null,
                    2
                  )
                )
              }
            >
              Use Example JSON
            </button>
            <button
              type="button"
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              onClick={() => onRunInputChange("{}")}
            >
              Set Empty Object
            </button>
            <button
              type="button"
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              onClick={() => {
                const parsed = runInputValidation.parsed;
                if (parsed === undefined && runInput.trim().length === 0) {
                  onRunInputChange("{}");
                  return;
                }
                if (runInputValidation.error || parsed === undefined) {
                  return;
                }
                onRunInputChange(JSON.stringify(parsed, null, 2));
              }}
              disabled={runInputHasError}
            >
              Format JSON
            </button>
            <button
              type="button"
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              onClick={() => onRunInputChange("")}
              disabled={runInput.trim().length === 0}
            >
              Clear
            </button>
          </div>
          <p className="text-xs text-slate-500">
            Leave empty to submit default empty input <code>{`{}`}</code>.
          </p>
        </div>
        <div className={RUN_PANEL_LIST_CLASS}>
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
              <p className="muted">No runs loaded yet. Create one or use Debug → Run Ops.</p>
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
    </div>
  );
}
