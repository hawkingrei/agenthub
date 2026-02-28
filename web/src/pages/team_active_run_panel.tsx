import type { TeamRunRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import { TEAM_PANEL_REFRESH_BUTTON_CLASS, TEAM_PANEL_SECONDARY_BUTTON_CLASS } from "../ui/tailwind_classes";

type TeamActiveRunPanelProps = {
  run: TeamRunRecord;
  busy: string | null;
  canResumeRun: boolean;
  canRestartRun: boolean;
  onRefresh: () => void;
  onCancel: () => void;
  onResume: () => void;
  onRestart: () => void;
  formatTs: (value: number | null) => string;
  cardClassName: string;
  titleClassName: string;
  metaItemClassName: string;
};

export function TeamActiveRunPanel(props: TeamActiveRunPanelProps) {
  const {
    run,
    busy,
    canResumeRun,
    canRestartRun,
    onRefresh,
    onCancel,
    onResume,
    onRestart,
    formatTs,
    cardClassName,
    titleClassName,
    metaItemClassName,
  } = props;

  return (
    <div className={cardClassName}>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <h3 className={titleClassName}>Active Run</h3>
        <div className="flex flex-wrap items-center gap-2">
          <button
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            title="Refresh active run"
            aria-label="Refresh active run"
            onClick={onRefresh}
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </button>
          <button
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={onCancel}
            disabled={busy === "cancel-run" || run.status === "canceled"}
          >
            Cancel Run
          </button>
          <button
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={onResume}
            disabled={busy === "resume-run" || !canResumeRun}
            title={
              canResumeRun
                ? "Resume a failed/canceled run"
                : "Resume is available for failed/canceled runs"
            }
          >
            Resume Run
          </button>
          <button
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={onRestart}
            disabled={busy === "restart-run" || !canRestartRun}
            title={
              canRestartRun
                ? "Create a fresh run from the same context/input"
                : "Restart is available for completed/failed/canceled runs"
            }
          >
            Restart Run
          </button>
        </div>
      </div>
      <div className="mt-3 grid min-w-0 gap-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-3">
        <span className={metaItemClassName}>
          <strong>ID:</strong> <code>{run.id}</code>
        </span>
        <span className={metaItemClassName}>
          <strong>Status:</strong>{" "}
          <StatusBadge
            label={run.status}
            tone={resolveTeamRunStatusTone(run.status)}
            className="team-status"
            title={`run status: ${run.status}`}
          />
        </span>
        <span className={metaItemClassName}>
          <strong>Context:</strong> {run.context_id}
        </span>
        <span className={metaItemClassName}>
          <strong>Created:</strong> {formatTs(run.created_at)}
        </span>
        <span className={metaItemClassName}>
          <strong>Started:</strong> {formatTs(run.started_at)}
        </span>
        <span className={metaItemClassName}>
          <strong>Ended:</strong> {formatTs(run.ended_at)}
        </span>
      </div>
    </div>
  );
}
