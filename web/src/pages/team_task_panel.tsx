import React from "react";
import {
  TeamConversationMessageRecord,
  TeamTaskRecord,
} from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { renderMarkdown } from "../markdown";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  normalizeTeamMemberLifecycle,
  normalizeTeamMemberWorkStatus,
} from "./team_member_status_strip";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamTaskPanelProps = {
  tasks: TeamTaskRecord[];
  tasksLoading: boolean;
  selectedTaskId: string;
  onSelectedTaskIdChange: (value: string) => void;
  onRefreshTasks: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: () => Promise<void> | void;
  onRefreshMessages: () => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
  memberLiveStates?: TeamMemberLiveState[];
  messagesLoading: boolean;
  busy: string | null;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

const TEAM_TASK_COMPOSER_PANEL_CLASS =
  "mt-3 rounded-xl border border-ui-border bg-ui-surface-soft/60 p-3";
const TEAM_TASK_SHORTCUT_CLASS = "text-ui-xs text-ui-text-muted";
const TEAM_TASK_MESSAGE_EMPTY_CLASS =
  "rounded-lg border border-dashed border-ui-border-strong bg-ui-surface px-3 py-2 text-ui-sm text-ui-text-muted";
const TEAM_TASK_SUBSECTION_TITLE_CLASS = "mt-3 text-ui-sm font-semibold text-ui-text-primary";
const TEAM_TASK_ACTIVITY_LIST_CLASS =
  "mt-2 max-h-[420px] overflow-y-auto rounded-xl border border-ui-border bg-ui-surface-soft/40 p-2";
const TEAM_TASK_ACTIVITY_STACK_CLASS = "flex w-full flex-col gap-2";
const TEAM_TASK_ACTIVITY_ITEM_CLASS = "rounded-lg border border-ui-border bg-ui-surface px-3 py-2 shadow-sm";
const TEAM_TASK_ACTIVITY_META_ROW_CLASS =
  "mono mb-1 flex flex-wrap items-center gap-2 text-xs text-ui-text-muted";
const TEAM_TASK_ACTIVITY_BADGE_CLASS = "rounded-full border border-ui-border bg-ui-surface px-2 py-0.5";
const TEAM_TASK_ACTIVITY_STREAM_CONVERSATION_CLASS =
  "rounded-full border border-brand-primary/30 bg-brand-primary/10 px-2 py-0.5 text-[11px] font-semibold text-brand-primary";
const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "mt-2 rounded-md border border-ui-border bg-ui-surface-soft px-3 py-2 text-sm leading-6 text-ui-text-primary";

function resolveWorkTone(status: ReturnType<typeof normalizeTeamMemberWorkStatus>): StatusTone {
  if (status === "working") return "active";
  if (status === "pending") return "warning";
  if (status === "blocked") return "danger";
  if (status === "done") return "active";
  if (status === "idle") return "inactive";
  return "neutral";
}

function resolveLifecycleTone(
  lifecycle: ReturnType<typeof normalizeTeamMemberLifecycle>
): StatusTone {
  if (lifecycle === "working") return "active";
  if (lifecycle === "idle") return "warning";
  if (lifecycle === "stopped") return "inactive";
  if (lifecycle === "missing") return "danger";
  return "neutral";
}

function resolveMessageText(
  message: TeamConversationMessageRecord,
  toPrettyJson: (value: unknown) => string
): string {
  if (
    typeof message.payload === "object" &&
    message.payload !== null &&
    "type" in message.payload &&
    (message.payload as { type?: unknown }).type === "chat_message" &&
    "text" in message.payload
  ) {
    return String((message.payload as { text?: unknown }).text ?? "");
  }
  return toPrettyJson(message.payload);
}

export function TeamTaskPanel(props: TeamTaskPanelProps) {
  const {
    tasks,
    tasksLoading,
    selectedTaskId,
    onSelectedTaskIdChange,
    onRefreshTasks,
    messageDraft,
    onMessageDraftChange,
    onSendMessage,
    onRefreshMessages,
    messages,
    memberLiveStates = [],
    messagesLoading,
    busy,
    formatTs,
    toPrettyJson,
  } = props;
  const canSendMessage = messageDraft.trim().length > 0 && busy !== "send-task-message";
  const liveStateByMemberId = React.useMemo(
    () => new Map(memberLiveStates.map((member) => [member.member_id, member])),
    [memberLiveStates]
  );
  const waterfallItems = React.useMemo(() => {
    return messages
      .map((message) => ({
        key: `conversation-${message.message_id}`,
        sequence: message.message_id,
        createdAt: message.created_at,
        fromActorId: message.from_actor_id,
        toActorId: message.to_actor_id ?? null,
        routeOrStatus: message.route,
        markdownText: resolveMessageText(message, toPrettyJson),
        renderedHtml: "",
      }))
      .sort((left, right) => {
        if (left.createdAt !== right.createdAt) {
          return left.createdAt - right.createdAt;
        }
        if (left.sequence !== right.sequence) {
          return left.sequence - right.sequence;
        }
        return left.key.localeCompare(right.key);
      })
      .map((item) => ({
        ...item,
        renderedHtml: renderMarkdown(item.markdownText),
      }));
  }, [messages, toPrettyJson]);

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Human - Team Conversation</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            type="button"
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading || busy === "refresh-task"}
            title="Refresh tasks"
            aria-label="Refresh tasks"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh Tasks</span>
          </button>
        </div>
      </div>

      <p className={`mb-3 ${TEAM_MUTED_TEXT_CLASS}`}>
        Human messages are stored once in shared team conversation and routed through team mailbox.
        Use @member_id to target specific agents; without @ the message is broadcast to all members.
      </p>

      <div className="mt-3 grid gap-2 lg:grid-cols-2">
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={selectedTaskId}
          onChange={(event) => onSelectedTaskIdChange(event.target.value)}
        >
          <option value="">Select task conversation (optional)</option>
          {tasks.map((task) => (
            <option key={task.id} value={task.id}>
              {task.title}
            </option>
          ))}
        </select>
        <button
          type="button"
          className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          onClick={() => {
            void onRefreshMessages();
          }}
          disabled={!selectedTaskId || messagesLoading || busy === "refresh-task-messages"}
        >
          Refresh Messages
        </button>
      </div>

      <div className={TEAM_TASK_SUBSECTION_TITLE_CLASS}>Conversation Stream</div>
      <div className={TEAM_TASK_ACTIVITY_LIST_CLASS}>
        <div className={TEAM_TASK_ACTIVITY_STACK_CLASS}>
          {waterfallItems.map((item) => {
            const state = liveStateByMemberId.get(item.fromActorId);
            const workStatus = state ? normalizeTeamMemberWorkStatus(state) : "unknown";
            const lifecycle = state ? normalizeTeamMemberLifecycle(state) : "unknown";
            return (
              <div key={item.key} className={TEAM_TASK_ACTIVITY_ITEM_CLASS}>
                <div className={TEAM_TASK_ACTIVITY_META_ROW_CLASS}>
                  <span className={TEAM_TASK_ACTIVITY_STREAM_CONVERSATION_CLASS}>conversation</span>
                  <span className={TEAM_TASK_ACTIVITY_BADGE_CLASS}>
                    seq={item.sequence}
                  </span>
                  <span className={TEAM_TASK_ACTIVITY_BADGE_CLASS}>
                    {item.fromActorId} -&gt; {item.toActorId ?? "-"}
                  </span>
                  <span className={TEAM_TASK_ACTIVITY_BADGE_CLASS}>{item.routeOrStatus}</span>
                  <span>{formatTs(item.createdAt)}</span>
                </div>
                {state && (
                  <div className="mb-2 flex flex-wrap items-center gap-2">
                    <StatusBadge
                      label={`work:${workStatus}`}
                      tone={resolveWorkTone(workStatus)}
                      className="team-status"
                      title={`work status: run=${state.run_status} step=${state.step_status}`}
                    />
                    <StatusBadge
                      label={`agent:${lifecycle}`}
                      tone={resolveLifecycleTone(lifecycle)}
                      className="team-status"
                      title={`agent status: ${state.lifecycle_status}`}
                    />
                  </div>
                )}
                <div
                  className={TEAM_TASK_ACTIVITY_BODY_CLASS}
                  dangerouslySetInnerHTML={{ __html: item.renderedHtml }}
                />
              </div>
            );
          })}
          {messagesLoading && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              Loading conversation...
            </div>
          )}
          {!messagesLoading && waterfallItems.length === 0 && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              No conversation messages yet.
            </div>
          )}
        </div>
      </div>

      <div className={TEAM_TASK_COMPOSER_PANEL_CLASS}>
        <p className={`mb-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Message is visible to the whole team. Workers should reply when mentioned, correcting
          leader, adding critical context, or reporting new findings.
        </p>
        <p className={`mb-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          When no task is selected, messages are delivered to run mailbox only. Leader can define
          tasks later and persist structured conversation there.
        </p>
        <textarea
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={3}
          placeholder="Type planning message for the team (e.g. @worker-1 @worker-2 please verify)"
          value={messageDraft}
          onChange={(event) => onMessageDraftChange(event.target.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canSendMessage) {
              event.preventDefault();
              void onSendMessage();
            }
          }}
        />
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
            onClick={() => {
              void onSendMessage();
            }}
            disabled={!canSendMessage}
          >
            Send Message
          </button>
          <span className={TEAM_TASK_SHORTCUT_CLASS}>shortcut: Ctrl/Cmd + Enter</span>
        </div>
      </div>
    </div>
  );
}
