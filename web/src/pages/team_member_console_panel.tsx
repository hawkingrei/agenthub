import React from "react";
import {
  AgentEvent,
  TeamMemberSnapshot,
  TeamRunEventRecord,
  TeamRunSnapshotRecord,
} from "../api";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

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

const MEMBER_CONSOLE_DETAIL_CLASS =
  "teams-step-body mono rounded-xl border border-slate-200 bg-slate-50/70 p-3";
const MEMBER_CONSOLE_DETAIL_GRID_CLASS = "grid gap-2 md:grid-cols-2";
const MEMBER_CONSOLE_DETAIL_ITEM_CLASS = "rounded-lg border border-slate-200 bg-white p-2";
const MEMBER_CONSOLE_DETAIL_LABEL_CLASS = "text-[11px] font-semibold uppercase tracking-wide text-slate-500";
const MEMBER_CONSOLE_DETAIL_VALUE_CLASS =
  "mono mt-1 block min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-slate-800";
const MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS =
  "mono mt-1 block min-w-0 whitespace-pre-wrap break-words text-slate-800";
const MEMBER_CONSOLE_LIST_CLASS = "teams-event-list rounded-xl border border-slate-200 bg-slate-50/50 p-3";
const MEMBER_CONSOLE_LIST_ITEM_CLASS = "rounded-lg border border-slate-200 bg-white p-2";
const MEMBER_CONSOLE_EVENT_HEAD_CLASS =
  "teams-event-head mb-1 flex items-center gap-2 text-xs text-slate-600";
const MEMBER_CONSOLE_EMPTY_TEXT_CLASS = "muted text-sm text-slate-600";

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
  const mcpSkills =
    selectedMemberSnapshot?.skills.filter((skill) => {
      const normalized = skill.trim().toLowerCase();
      return (
        normalized.includes("mcp") ||
        normalized.includes("actor-mailbox") ||
        normalized.includes("actor-runtime")
      );
    }) ?? [];

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Member Console</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            onClick={() => {
              void onRefresh();
            }}
            disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
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
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          >
            Load Older
          </button>
        </div>
      </div>

      <div className="form-row">
        <select
          className={TEAM_PANEL_INPUT_CLASS}
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
        <div className={MEMBER_CONSOLE_DETAIL_CLASS}>
          <div className={MEMBER_CONSOLE_DETAIL_GRID_CLASS}>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>member_id</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>
                {selectedMemberSnapshot.member_id}
              </span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>role</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>{selectedMemberSnapshot.role}</span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>model</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>{selectedMemberSnapshot.model ?? "-"}</span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>status</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>{selectedMemberSnapshot.status}</span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>session_status</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>
                {selectedMemberSnapshot.session_status ?? "-"}
              </span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>remote_task_id</div>
              <span className={MEMBER_CONSOLE_DETAIL_VALUE_CLASS}>
                {selectedMemberSnapshot.latest_step?.remote_task_id ?? "-"}
              </span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>skills</div>
              <span className={MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS}>
                {selectedMemberSnapshot.skills.length > 0 ? selectedMemberSnapshot.skills.join(", ") : "-"}
              </span>
            </div>
            <div className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>mcp_skills</div>
              <span className={MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS}>
                {mcpSkills.length > 0 ? mcpSkills.join(", ") : "-"}
              </span>
            </div>
          </div>
          <details className="rounded-lg border border-slate-200 bg-white p-2">
            <summary className="cursor-pointer text-[11px] font-semibold uppercase tracking-wide text-slate-500">
              prompt
            </summary>
            <pre className={MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS}>
              {selectedMemberSnapshot.prompt ?? "-"}
            </pre>
          </details>
        </div>
      )}

      {!selectedMemberSnapshot && (
        <p className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}>
          Showing latest {previewLimit} run records. Select a member for full member history.
        </p>
      )}

      {selectedMemberSnapshot && !selectedMemberSnapshot.latest_step?.remote_task_id && (
        <p className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}>
          Selected member has no associated session yet.
        </p>
      )}

      {selectedMemberSnapshot &&
        selectedMemberSnapshot.latest_step?.remote_task_id &&
        memberEvents.length === 0 && (
          <p className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}>No member events yet.</p>
        )}

      {!selectedMemberSnapshot && displayedRunEvents.length === 0 && (
        <p className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}>No run records yet.</p>
      )}

      {selectedMemberSnapshot && (
        <ul className={MEMBER_CONSOLE_LIST_CLASS}>
          {memberEvents.map((event) => (
            <li key={event.event_id} className={MEMBER_CONSOLE_LIST_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_EVENT_HEAD_CLASS}>
                <span className="mono">#{event.event_id}</span>
                <span>{event.stream}</span>
                <span>{formatTs(event.ts)}</span>
              </div>
              <pre className={TEAM_PANEL_PRE_CLASS}>{event.message}</pre>
            </li>
          ))}
        </ul>
      )}

      {!selectedMemberSnapshot && (
        <ul className={MEMBER_CONSOLE_LIST_CLASS}>
          {displayedRunEvents.map((event) => (
            <li key={event.event_id} className={MEMBER_CONSOLE_LIST_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_EVENT_HEAD_CLASS}>
                <span className="mono">#{event.event_id}</span>
                <span>{event.event_type}</span>
                <span>{formatTs(event.ts)}</span>
              </div>
              <pre className={TEAM_PANEL_PRE_CLASS}>{toPrettyJson(event.payload)}</pre>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
