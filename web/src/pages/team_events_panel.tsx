import React from "react";
import { TeamRunEventRecord } from "../api";

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

const EVENTS_CARD_CLASS =
  "card rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur";
const EVENTS_TOOLBAR_CLASS = "toolbar mb-3 flex items-center justify-between gap-2";
const EVENTS_TOOLBAR_ACTIONS_CLASS = "actions flex items-center gap-2";
const EVENTS_SECONDARY_BUTTON_CLASS =
  "rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-900 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60";
const EVENTS_LIST_CLASS = "teams-event-list rounded-xl border border-slate-200 bg-slate-50/50 p-3";

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
    <div className={EVENTS_CARD_CLASS}>
      <div className={EVENTS_TOOLBAR_CLASS}>
        <h3>Run Events</h3>
        <div className={EVENTS_TOOLBAR_ACTIONS_CLASS}>
          <label className="checkbox">
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
            className={EVENTS_SECONDARY_BUTTON_CLASS}
          >
            Refresh
          </button>
          <button
            onClick={() => {
              void onLoadOlderEvents();
            }}
            disabled={previewMode || eventsLoading || !eventsHasMore || oldestEventId == null}
            className={EVENTS_SECONDARY_BUTTON_CLASS}
          >
            Load Older
          </button>
        </div>
      </div>
      {previewMode && (
        <p className="muted">
          Showing latest {previewLimit} records. For full event history, select a member in the
          Member Console tab.
        </p>
      )}
      {displayedRunEvents.length === 0 && <p className="muted">No events.</p>}
      <ul className={EVENTS_LIST_CLASS}>
        {displayedRunEvents.map((event) => (
          <li key={event.event_id}>
            <div className="teams-event-head">
              <span className="mono">#{event.event_id}</span>
              <span>{event.event_type}</span>
              <span>{formatTs(event.ts)}</span>
            </div>
            <pre className="mono">{toPrettyJson(event.payload)}</pre>
          </li>
        ))}
      </ul>
    </div>
  );
}
