import React from "react";
import { TeamRunEventRecord } from "../api";
import {
  TEAM_PANEL_ICON_BUTTON_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
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

const EVENTS_LIST_CLASS = "teams-event-list rounded-xl border border-slate-200 bg-slate-50/50 p-3";
const EVENTS_CHECKBOX_LABEL_CLASS = "checkbox inline-flex items-center gap-2 text-sm text-slate-700";
const EVENTS_EMPTY_TEXT_CLASS = "muted text-sm text-slate-600";
const EVENTS_ITEM_CLASS = "rounded-lg border border-slate-200 bg-white p-2";
const EVENTS_ITEM_HEAD_CLASS = "teams-event-head mb-1 flex items-center gap-2 text-xs text-slate-600";

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
    <div className={TEAM_PANEL_CARD_CLASS}>
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
            className={TEAM_PANEL_ICON_BUTTON_CLASS}
            title="Refresh events"
            aria-label="Refresh events"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
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
        <p className={EVENTS_EMPTY_TEXT_CLASS}>
          Showing latest {previewLimit} records. For full event history, select a member in the
          Member Console tab.
        </p>
      )}
      {displayedRunEvents.length === 0 && <p className={EVENTS_EMPTY_TEXT_CLASS}>No events.</p>}
      <ul className={EVENTS_LIST_CLASS}>
        {displayedRunEvents.map((event) => (
          <li key={event.event_id} className={EVENTS_ITEM_CLASS}>
            <div className={EVENTS_ITEM_HEAD_CLASS}>
              <span className="mono">#{event.event_id}</span>
              <span>{event.event_type}</span>
              <span>{formatTs(event.ts)}</span>
            </div>
            <pre className={TEAM_PANEL_PRE_CLASS}>{toPrettyJson(event.payload)}</pre>
          </li>
        ))}
      </ul>
    </div>
  );
}
