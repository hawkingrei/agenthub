import React from "react";
import { TeamRunEventRecord } from "../api";
import { ActionButton, EmptyState, InlineNotice, PanelHeader, SurfaceCard } from "../ui/primitives";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  EVENTS_LIST_CLASS,
  EVENTS_ITEM_CLASS,
  EVENTS_ITEM_HEAD_CLASS,
} from "../ui/tailwind_classes";

type TeamEventsPanelProps = {
  eventsAutoRefresh: boolean;
  onEventsAutoRefreshChange: (next: boolean) => void;
  onRefreshEvents: () => Promise<void> | void;
  onLoadOlderEvents: () => Promise<void> | void;
  eventsLoading: boolean;
  previewMode: boolean;
  previewLimit: number;
  eventsHasMore: boolean;
  oldestEventId: number | null;
  displayedRunEvents: TeamRunEventRecord[];
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

const EVENTS_CHECKBOX_LABEL_CLASS =
  "checkbox inline-flex items-center gap-2 text-[13px] text-notion-text font-medium cursor-pointer";

function TeamEventsPanelImpl(props: TeamEventsPanelProps) {
  const {
    eventsAutoRefresh,
    onEventsAutoRefreshChange,
    onRefreshEvents,
    onLoadOlderEvents,
    eventsLoading,
    previewMode,
    previewLimit,
    eventsHasMore,
    oldestEventId,
    displayedRunEvents,
    formatTs,
    toPrettyJson,
  } = props;

  return (
    <SurfaceCard className="p-4">
      <PanelHeader
        title="Run Events"
        titleClassName={TEAM_PANEL_TITLE_CLASS}
        actions={
          <div className="flex flex-wrap items-center gap-2">
          <label className={EVENTS_CHECKBOX_LABEL_CLASS}>
            <input
              type="checkbox"
              checked={eventsAutoRefresh}
              onChange={(event) => onEventsAutoRefreshChange(event.target.checked)}
            />
            Auto refresh
          </label>
          <ActionButton
            tone="secondary"
            size="sm"
            onClick={() => {
              void onRefreshEvents();
            }}
            disabled={eventsLoading}
            title="Refresh events"
            aria-label="Refresh events"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </ActionButton>
          <ActionButton
            tone="secondary"
            size="sm"
            onClick={() => {
              void onLoadOlderEvents();
            }}
            disabled={previewMode || eventsLoading || !eventsHasMore || oldestEventId == null}
          >
            Load Older
          </ActionButton>
          </div>
        }
      />
      {previewMode && (
        <InlineNotice tone="info" className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Showing latest {previewLimit} records.
        </InlineNotice>
      )}
      {displayedRunEvents.length === 0 && (
        <EmptyState className="mt-3" body="No events." />
      )}
      <ul className={EVENTS_LIST_CLASS}>
        {displayedRunEvents.map((event) => (
          <li key={event.event_id} className={EVENTS_ITEM_CLASS}>
            <div className={EVENTS_ITEM_HEAD_CLASS}>
              <span className="text-notion-accent font-bold">#{event.event_id}</span>
              <span className="text-notion-text">{event.event_type}</span>
              <span aria-hidden="true">·</span>
              <span>{formatTs(event.ts)}</span>
            </div>
            <pre className={`${TEAM_PANEL_PRE_CLASS} mt-2 text-[12px] bg-notion-sidebar/30 border-notion-border/50`}>{toPrettyJson(event.payload)}</pre>
          </li>
        ))}
      </ul>
    </SurfaceCard>
  );
}

export const TeamEventsPanel = React.memo(TeamEventsPanelImpl);
TeamEventsPanel.displayName = "TeamEventsPanel";
