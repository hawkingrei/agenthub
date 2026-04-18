import React from "react";
import type { TeamRunRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import { ActionButton, ToolbarRow } from "../ui/primitives";

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

function TeamActiveRunPanelImpl(props: TeamActiveRunPanelProps) {
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
      <ToolbarRow className="mb-3">
        <h3 className={titleClassName}>Active Execution Run</h3>
        <div className="flex flex-wrap items-center gap-2">
          <ActionButton
            tone="secondary"
            size="sm"
            title="Refresh active execution run"
            aria-label="Refresh active execution run"
            onClick={onRefresh}
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </ActionButton>
          <ActionButton
            tone="secondary"
            size="sm"
            onClick={onCancel}
            disabled={busy === "cancel-run" || run.status === "canceled"}
          >
            Cancel Run
          </ActionButton>
          <ActionButton
            tone="secondary"
            size="sm"
            onClick={onResume}
            disabled={busy === "resume-run" || !canResumeRun}
            title={
              canResumeRun
                ? "Resume a failed/canceled run"
                : "Resume is available for failed/canceled runs"
            }
          >
            Resume Run
          </ActionButton>
          <ActionButton
            tone="secondary"
            size="sm"
            onClick={onRestart}
            disabled={busy === "restart-run" || !canRestartRun}
            title={
              canRestartRun
                ? "Create a fresh run from the same context/input"
                : "Restart is available for completed/failed/canceled runs"
            }
          >
            Restart Run
          </ActionButton>
        </div>
      </ToolbarRow>
      <div className="mt-3 grid min-w-0 gap-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-3">
        <span className={metaItemClassName}>
          <strong>ID:</strong> <code>{run.id}</code>
        </span>
        <span className={metaItemClassName}>
          <strong>Execution status:</strong>{" "}
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

export const TeamActiveRunPanel = React.memo(TeamActiveRunPanelImpl);
TeamActiveRunPanel.displayName = "TeamActiveRunPanel";
