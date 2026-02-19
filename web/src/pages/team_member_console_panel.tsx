import React from "react";
import {
  AgentEvent,
  TeamMemberSnapshot,
  TeamRunEventRecord,
  TeamRunSnapshotRecord,
} from "../api";

type TeamMemberConsolePanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  selectedMemberId: string;
  onSelectedMemberIdChange: (memberId: string) => void;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  displayedRunEvents: TeamRunEventRecord[];
  previewLimit: number;
  onRefresh: () => Promise<void> | void;
  onLoadOlder: () => Promise<void> | void;
  toPrettyJson: (value: unknown) => string;
  formatTs: (ts?: number | null) => string;
};

export function TeamMemberConsolePanel(props: TeamMemberConsolePanelProps) {
  const {
    snapshot,
    selectedMemberId,
    onSelectedMemberIdChange,
    selectedMemberSnapshot,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    eventsLoading,
    oldestMemberEventId,
    displayedRunEvents,
    previewLimit,
    onRefresh,
    onLoadOlder,
    toPrettyJson,
    formatTs,
  } = props;

  return (
    <div className="card">
      <div className="toolbar">
        <h3>Member Console</h3>
        <div className="actions">
          <button
            onClick={() => {
              void onRefresh();
            }}
            disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
          >
            Refresh
          </button>
          <button
            onClick={() => {
              void onLoadOlder();
            }}
            disabled={
              !selectedMemberSnapshot ||
              memberEventsLoading ||
              !memberEventsHasMore ||
              oldestMemberEventId == null
            }
          >
            Load Older
          </button>
        </div>
      </div>

      <div className="form-row">
        <select
          value={selectedMemberId}
          onChange={(event) => onSelectedMemberIdChange(event.target.value)}
        >
          <option value="">Select member</option>
          {snapshot?.members.map((member) => (
            <option key={member.member_id} value={member.member_id}>
              {member.member_id} ({member.role})
            </option>
          ))}
        </select>
      </div>

      {selectedMemberSnapshot && (
        <div className="teams-step-body mono">
          <div>member_id: {selectedMemberSnapshot.member_id}</div>
          <div>role: {selectedMemberSnapshot.role}</div>
          <div>model: {selectedMemberSnapshot.model ?? "-"}</div>
          <div>status: {selectedMemberSnapshot.status}</div>
          <div>session_status: {selectedMemberSnapshot.session_status ?? "-"}</div>
          <div>remote_task_id: {selectedMemberSnapshot.latest_step?.remote_task_id ?? "-"}</div>
          <div>
            skills: {selectedMemberSnapshot.skills.length > 0 ? selectedMemberSnapshot.skills.join(", ") : "-"}
          </div>
          <div>prompt: {selectedMemberSnapshot.prompt ?? "-"}</div>
        </div>
      )}

      {!selectedMemberSnapshot && (
        <p className="muted">
          Showing latest {previewLimit} run records. Select a member for full member history.
        </p>
      )}

      {selectedMemberSnapshot && !selectedMemberSnapshot.latest_step?.remote_task_id && (
        <p className="muted">Selected member has no associated session yet.</p>
      )}

      {selectedMemberSnapshot &&
        selectedMemberSnapshot.latest_step?.remote_task_id &&
        memberEvents.length === 0 && <p className="muted">No member events yet.</p>}

      {!selectedMemberSnapshot && displayedRunEvents.length === 0 && (
        <p className="muted">No run records yet.</p>
      )}

      {selectedMemberSnapshot && (
        <ul className="teams-event-list">
          {memberEvents.map((event) => (
            <li key={event.event_id}>
              <div className="teams-event-head">
                <span className="mono">#{event.event_id}</span>
                <span>{event.stream}</span>
                <span>{formatTs(event.ts)}</span>
              </div>
              <pre className="mono">{event.message}</pre>
            </li>
          ))}
        </ul>
      )}

      {!selectedMemberSnapshot && (
        <ul className="teams-event-list">
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
      )}
    </div>
  );
}
