import { NativeSelect } from "@mantine/core";
import { TeamDefinitionRecord, TeamRunRecord, TeamRunStatus } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import { ActionButton, PanelHeader, StatusPill, SurfaceCard } from "../ui/primitives";
import {
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_SECTION_TITLE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
} from "../ui/tailwind_classes";

type TeamRunStatusFilter = TeamRunStatus | "all";

type TeamRunStatusFilterOption = {
  value: TeamRunStatusFilter;
  label: string;
};

const RUN_PANEL_LIST_CLASS =
  "teams-run-list flex flex-col gap-4 rounded-xl border border-notion-border bg-notion-sidebar/10 p-4 sm:p-6";
const RUN_PANEL_LIST_ITEMS_CLASS = "teams-run-list-items flex max-h-80 flex-col gap-2 overflow-y-auto pr-1";
const RUN_PANEL_SUBTITLE_CLASS = "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";
const RUN_PANEL_HINT_TEXT_CLASS = "text-[13px] text-notion-text-muted italic";
const RUN_PANEL_FOOT_META_CLASS = "mono text-[10px] font-bold uppercase tracking-widest text-notion-text-muted opacity-70";

type TeamRunPanelProps = {
  selectedTeam: TeamDefinitionRecord;
  developerMode: boolean;
  busy: string | null;
  onDeleteTeam: () => Promise<void> | void;
  onStartRun: () => Promise<void> | void;
  canStartRun: boolean;
  runBlockedReason?: string | null;
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
    onStartRun,
    canStartRun,
    runBlockedReason,
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
    runsHasMore,
    selectedTeamId,
    onLoadMoreRuns,
  } = props;

  return (
    <SurfaceCard className="p-4 sm:p-6">
      <PanelHeader
        title={<div className={TEAM_SECTION_TITLE_CLASS}>{selectedTeam.name}</div>}
        actions={
          <>
            <StatusPill className="mono">{selectedTeam.id}</StatusPill>
          {developerMode && (
            <ActionButton
              tone="danger"
              size="sm"
              type="button"
              onClick={() => {
                void onDeleteTeam();
              }}
              disabled={busy === "delete-team"}
            >
              Delete Team
            </ActionButton>
          )}
          </>
        }
      />

      <div className={RUN_PANEL_LIST_CLASS}>
        <div className="flex flex-col gap-1">
          <p className={RUN_PANEL_SUBTITLE_CLASS}>Run Browser</p>
          <p className={TEAM_MUTED_TEXT_CLASS}>
            Team execution is agent-driven.
          </p>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-notion-border/50 pb-4">
          <span className={RUN_PANEL_HINT_TEXT_CLASS}>
            {runBlockedReason ?? "Start a new run for this team."}
          </span>
          <ActionButton
            tone="primary"
            size="md"
            onClick={() => {
              void onStartRun();
            }}
            disabled={busy === "create-run" || !selectedTeamId || !canStartRun}
            title={runBlockedReason ?? "Start a new run"}
            aria-label="Start run"
          >
            {busy === "create-run" ? "Starting..." : "Start Run"}
          </ActionButton>
        </div>

        <div className="teams-run-list-head mt-2 flex flex-wrap items-center justify-between gap-3">
          <h3 className="text-[13px] font-bold text-notion-text uppercase tracking-tight">Runs</h3>
          <div className="actions flex flex-wrap items-center gap-2">
            <NativeSelect
              className="w-full sm:w-[164px]"
              aria-label="Run status filter"
              value={runStatusFilter}
              onChange={(event) => onRunStatusFilterChange(event.currentTarget.value as TeamRunStatusFilter)}
              data={runStatusFilterOptions}
              size="xs"
              radius="sm"
            />
            <ActionButton
              tone="secondary"
              size="md"
              onClick={() => {
                void onRefreshRuns();
              }}
              disabled={runsLoading}
              title="Refresh runs"
              aria-label="Refresh runs"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            </ActionButton>
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
            <p className="text-[12px] text-state-warning-text italic">
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
              <div className="flex w-full items-center justify-between gap-2">
                <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} mono font-bold`}>{run.id}</span>
                <StatusBadge
                  label={run.status}
                  tone={resolveTeamRunStatusTone(run.status)}
                  className="team-status"
                  title={`run status: ${run.status}`}
                />
              </div>
              {run.summary && (
                <span className={`${TEAM_LIST_ITEM_META_CLASS} line-clamp-1`}>{run.summary}</span>
              )}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-notion-border/50">
          <span className={RUN_PANEL_FOOT_META_CLASS}>
            {visibleRuns.length} of {totalLoadedRunsForTeam}
          </span>
          <ActionButton
            tone="secondary"
            size="md"
            onClick={() => {
              void onLoadMoreRuns();
            }}
            disabled={runsLoading || !runsHasMore || !selectedTeamId}
          >
            {runsLoading ? "..." : "Load More"}
          </ActionButton>
        </div>
      </div>
    </SurfaceCard>
  );
}
