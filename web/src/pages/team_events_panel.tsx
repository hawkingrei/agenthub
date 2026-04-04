import React from "react";
import { TeamRunEventRecord } from "../api";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
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

export function TeamEventsPanel(props: TeamEventsPanelProps) {
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
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Run Events</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <label className={EVENTS_CHECKBOX_LABEL_CLASS}>
            <input
              type="checkbox"
              checked={eventsAutoRefresh}
              onChange={(event) => onEventsAutoRefreshChange(event.target.checked)}
            />
            Auto refresh
          </label>
          <button
            onClick={() => {
              void onRefreshEvents();
            }}
            disabled={eventsLoading}
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            title="Refresh events"
            aria-label="Refresh events"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </button>
          <button
            onClick={() => {
              void onLoadOlderEvents();
            }}
            disabled={previewMode || eventsLoading || !eventsHasMore || oldestEventId == null}
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          >
            Load Older
          </button>
        </div>
      </div>
      {previewMode && (
        <p className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Showing latest {previewLimit} records.
        </p>
      )}
      {displayedRunEvents.length === 0 && <p className={TEAM_MUTED_TEXT_CLASS}>No events.</p>}
      <ul className={EVENTS_LIST_CLASS}>
        {displayedRunEvents.map((event) => (
          <li key={event.event_id} className={EVENTS_ITEM_CLASS}>
            <div className={EVENTS_ITEM_HEAD_CLASS}>
              <span className="text-notion-accent font-bold">#{event.event_id}</span>
              <span className="text-notion-text">{event.event_type}</span>
              <span>·</span>
              <span>{formatTs(event.ts)}</span>
            </div>
            <pre className={`${TEAM_PANEL_PRE_CLASS} mt-2 text-[12px] bg-notion-sidebar/30 border-notion-border/50`}>{toPrettyJson(event.payload)}</pre>
          </li>
        ))}
      </ul>
    </div>
  );
}
