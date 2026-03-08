import React from "react";
import {
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
} from "../api";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  createDisplayNameLookup,
  type MentionCandidate,
  renderMarkdownWithMentions,
  resolveMentionDraftQuery,
  resolveChatMessageText,
  resolveDisplayName,
  type MentionDraftQuery,
} from "./team/mailbox_helpers";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamTaskPanelProps = {
  developerMode: boolean;
  tasksLoading: boolean;
  onRefreshTasks: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: (payload: { text: string; mentionActorIds: string[] }) => Promise<void> | void;
  onRefreshMessages: () => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
  mailboxMessages?: TeamActorMessageRecord[];
  seenByMessageId?: Record<number, string[]>;
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
const TEAM_TASK_ACTIVITY_ITEM_CLASS =
  "rounded-lg border border-ui-border bg-ui-surface px-3 py-3 shadow-sm";
const TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS =
  "flex items-start justify-between gap-3";
const TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS =
  "flex min-w-0 flex-wrap items-center gap-2";
const TEAM_TASK_ACTIVITY_AUTHOR_CLASS =
  "text-sm font-semibold text-ui-text-primary";
const TEAM_TASK_ACTIVITY_TIME_CLASS = "text-xs text-ui-text-muted";
const TEAM_TASK_ACTIVITY_REPLY_BADGE_CLASS =
  "rounded-full border border-ui-border bg-ui-surface-soft px-2 py-0.5 text-[11px] font-medium uppercase tracking-[0.16em] text-ui-text-muted";
const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "mt-2 text-sm leading-6 text-ui-text-primary";
const TEAM_TASK_ACTIVITY_DETAILS_CLASS =
  "mt-2 rounded-md border border-ui-border bg-ui-surface-soft/80";
const TEAM_TASK_ACTIVITY_DETAILS_BUTTON_CLASS =
  "mt-2 inline-flex items-center rounded-md border border-ui-border bg-ui-surface-soft px-2.5 py-1 text-xs font-medium text-ui-text-muted transition hover:border-ui-border-emphasis hover:bg-ui-surface";
const TEAM_TASK_ACTIVITY_DETAILS_GRID_CLASS =
  "grid gap-2 border-t border-ui-border px-3 py-2 text-xs text-ui-text-muted sm:grid-cols-2";
const TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS =
  "mono font-medium text-ui-text-secondary";
const TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS =
  "inline-flex items-center rounded-md border border-ui-border bg-ui-surface-soft px-2 py-1 text-[11px] font-medium text-ui-text-muted transition hover:border-ui-border-emphasis hover:text-ui-text-primary";
const TEAM_TASK_ACTIVITY_SEEN_META_CLASS = "mt-2 flex flex-wrap items-center gap-2";
const TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS =
  "mt-2 flex flex-wrap items-center gap-2 text-xs text-ui-text-muted";

function isSafeMentionLabel(value: string): boolean {
  return /^[A-Za-z0-9._:-]+$/.test(value);
}

function resolveMessageText(
  message: TeamConversationMessageRecord,
  toPrettyJson: (value: unknown) => string
): string {
  const chatText = resolveChatMessageText(message.payload);
  if (chatText !== null) {
    return chatText;
  }
  return toPrettyJson(message.payload);
}

function resolveMailboxMessageText(
  message: TeamActorMessageRecord,
  toPrettyJson: (value: unknown) => string
): string {
  const chatText = resolveChatMessageText(message.payload);
  if (chatText !== null) {
    return chatText;
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

function resolveThreadAuthorLabel(
  actorId: string,
  humanActorId: string,
  liveStateByMemberId: Map<string, TeamMemberLiveState>
): string {
  if (isHumanMailboxActor(actorId, humanActorId)) {
    return "You";
  }
  const state = liveStateByMemberId.get(actorId);
  const agentName = state?.agent_name?.trim();
  if (agentName) {
    return agentName;
  }
  return actorId;
}

function resolveMentionLabel(
  actorId: string,
  liveStateByMemberId: Map<string, TeamMemberLiveState>
): string {
  const state = liveStateByMemberId.get(actorId);
  const agentName = state?.agent_name?.trim();
  if (agentName && isSafeMentionLabel(agentName)) {
    return agentName;
  }
  return actorId;
}

export function TeamTaskPanel(props: TeamTaskPanelProps) {
  const {
    developerMode,
    tasksLoading,
    onRefreshTasks,
    messageDraft,
    onMessageDraftChange,
    onSendMessage,
    onRefreshMessages,
    messages,
    mailboxMessages = [],
    seenByMessageId = {},
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
  const [expandedItemKeys, setExpandedItemKeys] = React.useState<Record<string, boolean>>({});
  const [expandedSeenKeys, setExpandedSeenKeys] = React.useState<Record<string, boolean>>({});
  const liveStateByMemberId = React.useMemo(
    () => new Map(memberLiveStates.map((member) => [member.member_id, member])),
    [memberLiveStates]
  );
  const memberDisplayNamesById = React.useMemo(
    () =>
      createDisplayNameLookup(
        memberIds.map((memberId) => [memberId, resolveMentionLabel(memberId, liveStateByMemberId)])
      ),
    [liveStateByMemberId, memberIds]
  );

  const mentionCandidates = React.useMemo<MentionCandidate[]>(() => {
    const seen = new Set<string>();
    const items: MentionCandidate[] = [];
    for (const memberId of [...memberIds, ...memberLiveStates.map((member) => member.member_id)]) {
      const normalized = memberId.trim();
      if (!normalized || seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      items.push({
        actorId: normalized,
        label: resolveMentionLabel(normalized, liveStateByMemberId),
        aliases: [normalized],
      });
    }
    return items;
  }, [liveStateByMemberId, memberIds, memberLiveStates]);
  const filteredMentionCandidates = React.useMemo(() => {
    if (!activeMention) {
      return [];
    }
    const keyword = activeMention.keyword.trim().toLowerCase();
    return mentionCandidates
      .filter((candidate) =>
        keyword.length === 0
          ? true
          : [candidate.label, candidate.actorId, ...candidate.aliases].some((value) =>
              value.toLowerCase().startsWith(keyword)
            )
      )
      .slice(0, 8);
  }, [activeMention, mentionCandidates]);

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
    (candidate: MentionCandidate) => {
      if (!activeMention) {
        return;
      }
      const applied = applyMentionAtTag(messageDraft, activeMention, candidate.label);
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
  const sendCurrentMessage = React.useCallback(() => {
    const normalizedDraft = canonicalizeMentionDraft(messageDraft, mentionCandidates);
    if (!normalizedDraft.text.trim()) {
      return;
    }
    void onSendMessage({
      text: normalizedDraft.text,
      mentionActorIds: normalizedDraft.mentionActorIds,
    });
  }, [mentionCandidates, messageDraft, onSendMessage]);
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
        renderedHtml: renderMarkdownWithMentions(item.markdownText, memberDisplayNamesById),
      }));
  }, [humanActorId, mailboxMessages, memberDisplayNamesById, messages, toPrettyJson]);

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
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

      {threadOptionsOpen && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading}
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
            disabled={messagesLoading}
          >
            Refresh Thread
          </button>
        </div>
      )}

      <div className={TEAM_TASK_ACTIVITY_LIST_CLASS}>
        <div className={TEAM_TASK_ACTIVITY_STACK_CLASS}>
          {waterfallItems.map((item) => {
            const state = liveStateByMemberId.get(item.fromActorId);
            const authorLabel = resolveThreadAuthorLabel(
              item.fromActorId,
              humanActorId,
              liveStateByMemberId
            );
            return (
              <div key={item.key} className={TEAM_TASK_ACTIVITY_ITEM_CLASS}>
                <div className={TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS}>
                  <div className={TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS}>
                    <span className={TEAM_TASK_ACTIVITY_AUTHOR_CLASS}>{authorLabel}</span>
                    {item.streamLabel === "reply" && (
                      <span className={TEAM_TASK_ACTIVITY_REPLY_BADGE_CLASS}>reply</span>
                    )}
                  </div>
                  <span className={TEAM_TASK_ACTIVITY_TIME_CLASS}>{formatTs(item.createdAt)}</span>
                </div>
                <div
                  className={TEAM_TASK_ACTIVITY_BODY_CLASS}
                  dangerouslySetInnerHTML={{ __html: item.renderedHtml }}
                />
                {item.streamLabel === "conversation" && (
                  <div className={TEAM_TASK_ACTIVITY_SEEN_META_CLASS}>
                    {(() => {
                      const seenActorIds = seenByMessageId[item.sequence] ?? [];
                      if (seenActorIds.length === 0) {
                        return (
                          <span className="text-xs text-ui-text-muted">Seen by 0 agents</span>
                        );
                      }
                      const expanded = Boolean(expandedSeenKeys[item.key]);
                      return (
                        <>
                          <button
                            type="button"
                            className={TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS}
                            onClick={() =>
                              setExpandedSeenKeys((current) => ({
                                ...current,
                                [item.key]: !current[item.key],
                              }))
                            }
                            aria-expanded={expanded}
                          >
                            {`Seen by ${seenActorIds.length} agent${seenActorIds.length === 1 ? "" : "s"}`}
                          </button>
                          {expanded && (
                            <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                              {seenActorIds.map((actorId) => (
                                <span
                                  key={`${item.key}-${actorId}`}
                                  className="rounded-full border border-ui-border bg-ui-surface px-2 py-0.5"
                                >
                                  {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                                </span>
                              ))}
                            </div>
                          )}
                        </>
                      );
                    })()}
                  </div>
                )}
                {developerMode && (
                  <button
                    type="button"
                    className={TEAM_TASK_ACTIVITY_DETAILS_BUTTON_CLASS}
                    onClick={() =>
                      setExpandedItemKeys((current) => ({
                        ...current,
                        [item.key]: !current[item.key],
                      }))
                    }
                    aria-expanded={Boolean(expandedItemKeys[item.key])}
                  >
                    {expandedItemKeys[item.key] ? "Hide details" : "Show details"}
                  </button>
                )}
                {developerMode && expandedItemKeys[item.key] && (
                  <div className={TEAM_TASK_ACTIVITY_DETAILS_CLASS}>
                    <dl className={TEAM_TASK_ACTIVITY_DETAILS_GRID_CLASS}>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>source</dt>
                      <dd>{item.streamLabel}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>seq</dt>
                      <dd>{item.sequence}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>from</dt>
                      <dd className="mono">{item.fromActorId}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>to</dt>
                      <dd className="mono">{item.toActorId ?? "-"}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>route</dt>
                      <dd>{item.routeOrStatus}</dd>
                    </div>
                    {state && (
                      <>
                        <div>
                          <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>work</dt>
                          <dd>{state.run_status}/{state.step_status}</dd>
                        </div>
                        <div>
                          <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>agent</dt>
                          <dd>{state.lifecycle_status}</dd>
                        </div>
                        {state.current_work && (
                          <div className="sm:col-span-2">
                            <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>current_work</dt>
                            <dd>{state.current_work}</dd>
                          </div>
                        )}
                      </>
                    )}
                    </dl>
                  </div>
                )}
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
        <textarea
          ref={messageTextareaRef}
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={3}
          placeholder="Message #all"
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
            if (activeMention && filteredMentionCandidates.length > 0) {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActiveMentionIndex((prev) => (prev + 1) % filteredMentionCandidates.length);
                return;
              }
              if (event.key === "ArrowUp") {
                event.preventDefault();
                setActiveMentionIndex((prev) =>
                  prev === 0 ? filteredMentionCandidates.length - 1 : prev - 1
                );
                return;
              }
              if ((event.key === "Enter" || event.key === "Tab") && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                const selected =
                  filteredMentionCandidates[activeMentionIndex] ?? filteredMentionCandidates[0];
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
              sendCurrentMessage();
            }
          }}
        />
        {activeMention && filteredMentionCandidates.length > 0 && (
          <div className="mt-2 rounded-lg border border-ui-border bg-ui-surface shadow-sm">
            <div className="px-3 py-1 text-xs text-ui-text-muted">
              Select teammate mention (`@` without selection stays plain text)
            </div>
            <div className="max-h-44 overflow-auto py-1">
              {filteredMentionCandidates.map((candidate, index) => (
                <button
                  key={candidate.actorId}
                  type="button"
                  className={`flex w-full items-center justify-between px-3 py-1 text-left text-sm ${
                    index === activeMentionIndex
                      ? "bg-brand-primary/10 text-brand-primary"
                      : "text-ui-text-primary hover:bg-ui-surface-soft"
                  }`}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    applyMentionSelection(candidate);
                  }}
                >
                  <span>{candidate.label}</span>
                  <span className="text-[11px] text-ui-text-muted">{`@${candidate.label}`}</span>
                </button>
              ))}
            </div>
          </div>
        )}
        <div className={TEAM_TASK_COMPOSER_META_ROW_CLASS}>
          <span className={TEAM_TASK_SHORTCUT_CLASS}>
            {`Use @agent_name for direct replies · Ctrl/Cmd + Enter to send`}
          </span>
          <button
            type="button"
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
            onClick={() => {
              sendCurrentMessage();
            }}
            disabled={!canSendMessage}
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
