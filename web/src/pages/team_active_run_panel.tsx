import React from "react";
import type { TeamRunRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  ActionButton,
  KeyValueItem,
  KeyValueList,
  PanelHeader,
  SurfaceCard,
} from "../ui/primitives";

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
  } = props;

  return (
    <SurfaceCard className={cardClassName}>
      <PanelHeader
        className="mb-3"
        title="Active Execution Run"
        titleClassName={titleClassName}
        actions={
          <>
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
            Cancel Execution Run
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
            Resume Execution Run
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
            Restart Execution Run
          </ActionButton>
          </>
        }
      />
      <KeyValueList className="mt-3 gap-y-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-3">
        <KeyValueItem label="Execution run:" value={<code>{run.id}</code>} />
        <KeyValueItem
          label="Execution status:"
          value={
            <StatusBadge
              label={run.status}
              tone={resolveTeamRunStatusTone(run.status)}
              className="team-status"
              title={`run status: ${run.status}`}
            />
          }
        />
        <KeyValueItem label="Context:" value={run.context_id} />
        <KeyValueItem label="Created:" value={formatTs(run.created_at)} />
        <KeyValueItem label="Started:" value={formatTs(run.started_at)} />
        <KeyValueItem label="Ended:" value={formatTs(run.ended_at)} />
      </KeyValueList>
    </SurfaceCard>
  );
}

export const TeamActiveRunPanel = React.memo(TeamActiveRunPanelImpl);
TeamActiveRunPanel.displayName = "TeamActiveRunPanel";
