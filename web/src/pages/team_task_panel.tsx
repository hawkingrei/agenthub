import React from "react";
import {
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
  TeamTaskRecord,
} from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  applyMentionAtTag,
  renderMarkdownWithMentions,
  resolveMentionDraftQuery,
  type MentionDraftQuery,
} from "./team/mailbox_helpers";
import {
  normalizeTeamMemberLifecycle,
  normalizeTeamMemberWorkStatus,
} from "./team_member_status_strip";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
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
  mailboxMessages?: TeamActorMessageRecord[];
  humanActorId?: string;
  memberLiveStates?: TeamMemberLiveState[];
  memberIds?: string[];
  messagesLoading: boolean;
  busy: string | null;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

const TEAM_TASK_COMPOSER_PANEL_CLASS =
  "mt-4 flex flex-col gap-3 rounded-xl border border-ui-border-strong bg-ui-surface p-3 shadow-sm";
const TEAM_TASK_SHORTCUT_CLASS = "text-ui-xs text-ui-text-muted";
const TEAM_TASK_COMPOSER_META_ROW_CLASS =
  "flex flex-wrap items-center justify-between gap-2";
const TEAM_TASK_MESSAGE_EMPTY_CLASS =
  "rounded-lg border border-dashed border-ui-border-strong bg-ui-surface px-3 py-2 text-ui-sm text-ui-text-muted";
const TEAM_TASK_ACTIVITY_LIST_CLASS =
  "mt-3 min-h-[320px] max-h-[min(72vh,760px)] overflow-y-auto rounded-xl border border-ui-border bg-ui-surface-soft/40 p-2";
const TEAM_TASK_ACTIVITY_STACK_CLASS = "flex w-full flex-col gap-2";
const TEAM_TASK_ACTIVITY_ITEM_CLASS = "rounded-lg border border-ui-border bg-ui-surface px-3 py-2 shadow-sm";
const TEAM_TASK_ACTIVITY_META_ROW_CLASS =
  "mono mb-1 flex flex-wrap items-center gap-2 text-xs text-ui-text-muted";
const TEAM_TASK_ACTIVITY_BADGE_CLASS = "rounded-full border border-ui-border bg-ui-surface px-2 py-0.5";
const TEAM_TASK_ACTIVITY_STREAM_CONVERSATION_CLASS =
  "rounded-full border border-brand-primary/30 bg-brand-primary/10 px-2 py-0.5 text-[11px] font-semibold text-brand-primary";
const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "mt-2 rounded-md border border-ui-border bg-ui-surface-soft px-3 py-2 text-sm leading-6 text-ui-text-primary";

type ThreadWaterfallItem = {
  key: string;
  sequence: number;
  createdAt: number;
  fromActorId: string;
  toActorId: string | null;
  routeOrStatus: string;
  streamLabel: string;
  markdownText: string;
  renderedHtml: string;
};

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

function resolveMailboxMessageText(
  message: TeamActorMessageRecord,
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

function isHumanMailboxActor(actorId: string | null | undefined, humanActorId: string): boolean {
  const normalized = (actorId ?? "").trim();
  const human = humanActorId.trim();
  if (!normalized || !human) {
    return false;
  }
  return normalized === human || normalized.startsWith(`${human}:`);
}

export function TeamTaskPanel(props: TeamTaskPanelProps) {
  const {
    tasksLoading,
    onRefreshTasks,
    messageDraft,
    onMessageDraftChange,
    onSendMessage,
    onRefreshMessages,
    messages,
    mailboxMessages = [],
    humanActorId = "user",
    memberLiveStates = [],
    memberIds = [],
    messagesLoading,
    busy,
    formatTs,
    toPrettyJson,
  } = props;
  const messageTextareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const [activeMention, setActiveMention] = React.useState<MentionDraftQuery | null>(null);
  const [activeMentionIndex, setActiveMentionIndex] = React.useState(0);
  const [threadOptionsOpen, setThreadOptionsOpen] = React.useState(false);

  const mentionableMemberIds = React.useMemo(() => {
    const seen = new Set<string>();
    const ids: string[] = [];
    for (const memberId of [...memberIds, ...memberLiveStates.map((member) => member.member_id)]) {
      const normalized = memberId.trim();
      if (!normalized || seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      ids.push(normalized);
    }
    return ids;
  }, [memberIds, memberLiveStates]);
  const mentionCandidates = React.useMemo(() => {
    if (!activeMention) {
      return [];
    }
    const keyword = activeMention.keyword.trim().toLowerCase();
    return mentionableMemberIds
      .filter((memberId) =>
        keyword.length === 0 ? true : memberId.toLowerCase().startsWith(keyword)
      )
      .slice(0, 8);
  }, [activeMention, mentionableMemberIds]);

  const updateMentionQuery = React.useCallback((draft: string, cursor: number | null) => {
    if (cursor === null || Number.isNaN(cursor)) {
      setActiveMention(null);
      setActiveMentionIndex(0);
      return;
    }
    const next = resolveMentionDraftQuery(draft, cursor);
    setActiveMention(next);
    setActiveMentionIndex(0);
  }, []);

  const applyMentionSelection = React.useCallback(
    (actorId: string) => {
      if (!activeMention) {
        return;
      }
      const applied = applyMentionAtTag(messageDraft, activeMention, actorId);
      onMessageDraftChange(applied.text);
      setActiveMention(null);
      setActiveMentionIndex(0);
      requestAnimationFrame(() => {
        const textarea = messageTextareaRef.current;
        if (!textarea) {
          return;
        }
        textarea.focus();
        textarea.setSelectionRange(applied.cursor, applied.cursor);
      });
    },
    [activeMention, messageDraft, onMessageDraftChange]
  );

  const canSendMessage = messageDraft.trim().length > 0 && busy !== "send-task-message";
  const liveStateByMemberId = React.useMemo(
    () => new Map(memberLiveStates.map((member) => [member.member_id, member])),
    [memberLiveStates]
  );
  const waterfallItems = React.useMemo(() => {
    const conversationItems = messages
      .map((message) => ({
        key: `conversation-${message.message_id}`,
        sequence: message.message_id,
        createdAt: message.created_at,
        fromActorId: message.from_actor_id,
        toActorId: message.to_actor_id ?? null,
        routeOrStatus: message.route,
        streamLabel: "conversation",
        markdownText: resolveMessageText(message, toPrettyJson),
        renderedHtml: "",
      }));
    const conversationSignatures = new Set(
      conversationItems.map(
        (item) =>
          `${item.fromActorId}|${item.toActorId ?? "-"}|${item.createdAt}|${item.markdownText}`
      )
    );
    const mailboxReplyItems = mailboxMessages
      .filter((message) => {
        if (message.status !== "delivered") {
          return false;
        }
        if (!isHumanMailboxActor(message.to_actor_id, humanActorId)) {
          return false;
        }
        if (isHumanMailboxActor(message.from_actor_id, humanActorId)) {
          return false;
        }
        const text = resolveMailboxMessageText(message, toPrettyJson);
        return text.trim().length > 0;
      })
      .map((message) => ({
        key: `mailbox-${message.message_id}`,
        sequence: message.message_id,
        createdAt: message.created_at,
        fromActorId: message.from_actor_id,
        toActorId: message.to_actor_id ?? null,
        routeOrStatus: "mailbox",
        streamLabel: "reply",
        markdownText: resolveMailboxMessageText(message, toPrettyJson),
        renderedHtml: "",
      }))
      .filter((item) => {
        const signature = `${item.fromActorId}|${item.toActorId ?? "-"}|${item.createdAt}|${item.markdownText}`;
        return !conversationSignatures.has(signature);
      });
    return [...conversationItems, ...mailboxReplyItems]
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
        renderedHtml: renderMarkdownWithMentions(item.markdownText),
      }));
  }, [humanActorId, mailboxMessages, messages, toPrettyJson]);

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Thread</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            type="button"
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={() => setThreadOptionsOpen((current) => !current)}
            aria-expanded={threadOptionsOpen}
            aria-label="Toggle thread options"
            title="Thread options"
          >
            <i className="bi bi-three-dots" aria-hidden="true" />
          </button>
        </div>
      </div>

      <p className={`mb-3 ${TEAM_MUTED_TEXT_CLASS}`}>
        Shared team thread. Use `@` for directed messages; plain text stays broadcast.
      </p>

      {threadOptionsOpen && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading || busy === "refresh-task"}
            title="Refresh channel"
            aria-label="Refresh channel"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh Channel</span>
          </button>
          <button
            type="button"
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={() => {
              void onRefreshMessages();
            }}
            disabled={messagesLoading || busy === "refresh-task-messages"}
          >
            Refresh Thread
          </button>
        </div>
      )}

      <div className={TEAM_TASK_ACTIVITY_LIST_CLASS}>
        <div className={TEAM_TASK_ACTIVITY_STACK_CLASS}>
          {waterfallItems.map((item) => {
            const state = liveStateByMemberId.get(item.fromActorId);
            const workStatus = state ? normalizeTeamMemberWorkStatus(state) : "unknown";
            const lifecycle = state ? normalizeTeamMemberLifecycle(state) : "unknown";
            return (
              <div key={item.key} className={TEAM_TASK_ACTIVITY_ITEM_CLASS}>
                <div className={TEAM_TASK_ACTIVITY_META_ROW_CLASS}>
                  <span className={TEAM_TASK_ACTIVITY_STREAM_CONVERSATION_CLASS}>{item.streamLabel}</span>
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
              Loading thread...
            </div>
          )}
          {!messagesLoading && waterfallItems.length === 0 && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              No thread messages yet.
            </div>
          )}
        </div>
      </div>

      <div className={TEAM_TASK_COMPOSER_PANEL_CLASS}>
        <p className={TEAM_MUTED_TEXT_CLASS}>
          Broadcast by default. Add `@member_id` to direct one teammate.
        </p>
        <textarea
          ref={messageTextareaRef}
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={4}
          placeholder="Type planning message (use @ to pick teammate mention)"
          value={messageDraft}
          onChange={(event) => {
            const nextDraft = event.target.value;
            onMessageDraftChange(nextDraft);
            updateMentionQuery(nextDraft, event.target.selectionStart);
          }}
          onClick={(event) =>
            updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
          }
          onKeyUp={(event) =>
            updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
          }
          onBlur={() => {
            setTimeout(() => {
              setActiveMention(null);
              setActiveMentionIndex(0);
            }, 0);
          }}
          onKeyDown={(event) => {
            if (activeMention && mentionCandidates.length > 0) {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActiveMentionIndex((prev) => (prev + 1) % mentionCandidates.length);
                return;
              }
              if (event.key === "ArrowUp") {
                event.preventDefault();
                setActiveMentionIndex((prev) =>
                  prev === 0 ? mentionCandidates.length - 1 : prev - 1
                );
                return;
              }
              if ((event.key === "Enter" || event.key === "Tab") && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                const selected = mentionCandidates[activeMentionIndex] ?? mentionCandidates[0];
                if (selected) {
                  applyMentionSelection(selected);
                }
                return;
              }
              if (event.key === "Escape") {
                event.preventDefault();
                setActiveMention(null);
                setActiveMentionIndex(0);
                return;
              }
            }
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canSendMessage) {
              event.preventDefault();
              void onSendMessage();
            }
          }}
        />
        {activeMention && mentionCandidates.length > 0 && (
          <div className="mt-2 rounded-lg border border-ui-border bg-ui-surface shadow-sm">
            <div className="px-3 py-1 text-xs text-ui-text-muted">
              Select teammate mention (`@` without selection stays plain text)
            </div>
            <div className="max-h-44 overflow-auto py-1">
              {mentionCandidates.map((memberId, index) => (
                <button
                  key={memberId}
                  type="button"
                  className={`flex w-full items-center justify-between px-3 py-1 text-left text-sm ${
                    index === activeMentionIndex
                      ? "bg-brand-primary/10 text-brand-primary"
                      : "text-ui-text-primary hover:bg-ui-surface-soft"
                  }`}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    applyMentionSelection(memberId);
                  }}
                >
                  <span className="mono">{memberId}</span>
                  <span className="text-[11px] text-ui-text-muted">{`@${memberId}`}</span>
                </button>
              ))}
            </div>
          </div>
        )}
        <div className={TEAM_TASK_COMPOSER_META_ROW_CLASS}>
          <span className={TEAM_TASK_SHORTCUT_CLASS}>shortcut: Ctrl/Cmd + Enter</span>
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
        </div>
      </div>
    </div>
  );
}
