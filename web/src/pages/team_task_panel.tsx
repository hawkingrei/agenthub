import { HoverCard } from "@mantine/core";
import React from "react";
import { TeamConversationMessageRecord } from "../api";
import { ThreadRichText } from "../components/thread_rich_text";
import { windowConversation } from "../conversation";
import { deriveThreadJumpState, deriveThreadStickToBottom } from "../hooks/thread_viewport";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  createDisplayNameLookup,
  isHumanMailboxActor,
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
  TEAM_PANEL_TEXTAREA_CLASS,
} from "../ui/tailwind_classes";

type TeamTaskPanelProps = {
  developerMode: boolean;
  tasksLoading?: boolean;
  onRefreshTasks?: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: (payload: { text: string; mentionActorIds: string[] }) => Promise<void> | void;
  onRefreshMessages?: () => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
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
  "mt-2.5 flex flex-col gap-2 rounded-[12px] border border-black/6 bg-white/72 px-2.5 py-2";
const TEAM_TASK_SHORTCUT_CLASS = "text-ui-xs text-ui-text-muted";
const TEAM_TASK_COMPOSER_META_ROW_CLASS =
  "flex flex-wrap items-center justify-between gap-2";
const TEAM_TASK_MESSAGE_EMPTY_CLASS =
  "px-1 py-2 text-ui-sm text-ui-text-muted";
const TEAM_TASK_ACTIVITY_LIST_CLASS =
  "mt-2 min-h-[220px] max-h-[min(72vh,760px)] overflow-y-auto pr-1";
const TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS =
  "mt-2 min-h-[120px] overflow-y-auto pr-1";
const TEAM_TASK_ACTIVITY_SHELL_CLASS =
  "rounded-[12px] border border-black/6 bg-[rgba(255,255,255,0.68)] px-2.5 py-2";
const TEAM_TASK_ACTIVITY_STACK_CLASS =
  "flex w-full flex-col gap-2";
const TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS =
  "acp-bubble relative rounded-[12px] border px-2.5 py-2";
const TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS =
  `${TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS} border-[rgba(31,122,61,0.12)] bg-[rgba(31,122,61,0.04)] text-ui-text-primary`;
const TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS =
  `${TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS} border-black/6 bg-white/74 text-ui-text-primary`;
const TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS =
  "flex items-start justify-between gap-3";
const TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS =
  "flex min-w-0 flex-wrap items-center gap-2";
const TEAM_TASK_ACTIVITY_AUTHOR_CLASS =
  "text-sm font-semibold tracking-tight text-ui-text-primary";
const TEAM_TASK_ACTIVITY_TIME_CLASS =
  "text-[11px] font-medium uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "mt-2 overflow-hidden text-ui-text-primary";
const TEAM_TASK_ACTIVITY_DETAILS_CLASS =
  "mt-3 rounded-lg border border-ui-border/80 bg-ui-surface/70";
const TEAM_TASK_ACTIVITY_DETAILS_BUTTON_CLASS =
  "mt-2 inline-flex items-center rounded-md border border-ui-border bg-ui-surface-soft px-2.5 py-1 text-xs font-medium text-ui-text-muted transition hover:border-ui-border-emphasis hover:bg-ui-surface";
const TEAM_TASK_ACTIVITY_DETAILS_GRID_CLASS =
  "grid gap-2 border-t border-ui-border px-3 py-2 text-xs text-ui-text-muted sm:grid-cols-2";
const TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS =
  "mono font-medium text-ui-text-secondary";
const TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS =
  "inline-flex items-center rounded-full border border-black/6 bg-white/78 p-0.5 text-[11px] font-medium text-ui-text-muted transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft hover:text-ui-text-primary";
const TEAM_TASK_ACTIVITY_SEEN_META_CLASS = "absolute bottom-2.5 right-2.5 z-[1]";
const TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS =
  "mt-2 flex flex-wrap items-center gap-2 text-xs text-ui-text-muted";
const TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS =
  "inline-flex h-3 w-3 rounded-full border border-black/10 bg-[rgba(55,53,47,0.18)]";
const TEAM_TASK_ACTIVITY_SEEN_DIAL_CLASS =
  "relative inline-flex items-center justify-center overflow-hidden rounded-full align-middle shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06)]";
const TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS =
  "min-w-[220px] rounded-[12px] border border-black/6 bg-[rgba(252,251,247,0.98)] p-3 shadow-[0_8px_24px_rgba(15,23,42,0.08)]";
const TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS =
  "text-[11px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS =
  "mt-1 text-sm font-semibold tracking-tight text-ui-text-primary";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS = "mt-3";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS =
  "text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_JUMP_BUTTON_CLASS =
  "inline-flex items-center rounded-full border border-black/6 bg-white/82 px-2 py-0.5 text-[11px] font-medium text-ui-text-muted backdrop-blur transition hover:border-ui-border-emphasis hover:text-ui-text-primary";
const TEAM_TASK_TOP_JUMP_MIN_MESSAGES = 12;
const TEAM_TASK_TAIL_WINDOW_SIZE = 10;
const TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT = 116;

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
  if (agentName) {
    return agentName;
  }
  return actorId;
}

function resolveActivityItemClassName(
  actorId: string,
  humanActorId: string
): string {
  return isHumanMailboxActor(actorId, humanActorId)
    ? TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS
    : TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS;
}

type SeenProgressState = {
  readActorIds: string[];
  unreadActorIds: string[];
  totalCount: number;
  readCount: number;
  unreadCount: number;
  progress: number;
};

type SeenDialStyle = React.CSSProperties & {
  "--value": number;
  "--size": string;
  "--thickness": string;
};

function resolveSeenProgressState(
  seenActorIds: string[],
  memberIds: string[],
  authorActorId: string
): SeenProgressState {
  const normalizedReadActorIds: string[] = [];
  const seen = new Set<string>();
  for (const actorId of seenActorIds) {
    const normalized = actorId.trim();
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    normalizedReadActorIds.push(normalized);
  }

  const recipientActorIds: string[] = [];
  const recipientSet = new Set<string>();
  for (const memberId of memberIds) {
    const normalized = memberId.trim();
    if (!normalized || normalized === authorActorId || recipientSet.has(normalized)) {
      continue;
    }
    recipientSet.add(normalized);
    recipientActorIds.push(normalized);
  }
  for (const actorId of normalizedReadActorIds) {
    if (recipientSet.has(actorId)) {
      continue;
    }
    recipientSet.add(actorId);
    recipientActorIds.push(actorId);
  }

  const unreadActorIds = recipientActorIds.filter((actorId) => !seen.has(actorId));
  const totalCount = recipientActorIds.length;
  const readCount = normalizedReadActorIds.length;
  return {
    readActorIds: normalizedReadActorIds,
    unreadActorIds,
    totalCount,
    readCount,
    unreadCount: unreadActorIds.length,
    progress: totalCount > 0 ? Math.round((readCount / totalCount) * 100) : 0,
  };
}

function TeamTaskPanelImpl(props: TeamTaskPanelProps) {
  const {
    developerMode,
    messageDraft,
    onMessageDraftChange,
    onSendMessage,
    messages,
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
  const [expandedItemKeys, setExpandedItemKeys] = React.useState<Record<string, boolean>>({});
  const activityListRef = React.useRef<HTMLDivElement | null>(null);
  const lastActivityScrollTopRef = React.useRef<number | null>(null);
  const [stickToBottom, setStickToBottom] = React.useState(true);
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
  const orderedMessages = React.useMemo(
    () =>
      [...messages].sort((left, right) => {
        if (left.created_at !== right.created_at) {
          return left.created_at - right.created_at;
        }
        if (left.message_id !== right.message_id) {
          return left.message_id - right.message_id;
        }
        return left.from_actor_id.localeCompare(right.from_actor_id);
      }),
    [messages]
  );
  const activityWindow = React.useMemo(
    () => windowConversation(orderedMessages, stickToBottom, TEAM_TASK_TAIL_WINDOW_SIZE),
    [orderedMessages, stickToBottom]
  );
  const visibleWaterfallItems = React.useMemo(
    () =>
      activityWindow.items.map((message) => {
        return {
          key: `conversation-${message.message_id}`,
          sequence: message.message_id,
          createdAt: message.created_at,
          fromActorId: message.from_actor_id,
          toActorId: message.to_actor_id ?? null,
          routeOrStatus: message.route,
          streamLabel: "conversation",
          text: resolveMessageText(message, toPrettyJson),
        };
      }),
    [activityWindow.items, toPrettyJson]
  );
  const hiddenWaterfallCount = activityWindow.offset;
  const hiddenWaterfallSpacerHeight = React.useMemo(() => {
    if (!stickToBottom || hiddenWaterfallCount <= 0) {
      return 0;
    }
    return hiddenWaterfallCount * TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT;
  }, [hiddenWaterfallCount, stickToBottom]);
  const activityListClassName =
    messagesLoading || orderedMessages.length > 0
      ? TEAM_TASK_ACTIVITY_LIST_CLASS
      : TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS;
  const latestWaterfallKey =
    orderedMessages.length > 0
      ? `conversation-${orderedMessages[orderedMessages.length - 1]?.message_id ?? "empty"}`
      : "empty";
  const activityJumpState = React.useMemo(
    () =>
      deriveThreadJumpState({
        active: orderedMessages.length > 0,
        stickToBottom,
        pendingCount: 0,
      }),
    [orderedMessages.length, stickToBottom]
  );
  const renderTeamMessageHtml = React.useCallback(
    (text: string) => renderMarkdownWithMentions(text, memberDisplayNamesById),
    [memberDisplayNamesById]
  );

  const scrollActivityToBottom = React.useCallback(() => {
    const node = activityListRef.current;
    if (!node) {
      return;
    }
    node.scrollTop = node.scrollHeight;
    lastActivityScrollTopRef.current = node.scrollTop;
  }, []);
  const scrollActivityToTop = React.useCallback(() => {
    const node = activityListRef.current;
    if (!node) {
      return;
    }
    node.scrollTop = 0;
    lastActivityScrollTopRef.current = node.scrollTop;
  }, []);

  React.useEffect(() => {
    if (messagesLoading || orderedMessages.length === 0 || !stickToBottom) {
      return;
    }
    const handle = window.requestAnimationFrame(() => {
      scrollActivityToBottom();
    });
    return () => {
      window.cancelAnimationFrame(handle);
    };
  }, [latestWaterfallKey, messagesLoading, orderedMessages.length, scrollActivityToBottom, stickToBottom]);

  const handleActivityScroll = React.useCallback(() => {
    const node = activityListRef.current;
    if (!node) {
      return;
    }
    const nextStickToBottom = deriveThreadStickToBottom({
      scrollHeight: node.scrollHeight,
      scrollTop: node.scrollTop,
      clientHeight: node.clientHeight,
      wasStickToBottom: stickToBottom,
      previousScrollTop: lastActivityScrollTopRef.current,
      threshold: 24,
    });
    lastActivityScrollTopRef.current = node.scrollTop;
    if (nextStickToBottom !== stickToBottom) {
      setStickToBottom(nextStickToBottom);
    }
  }, [stickToBottom]);

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div
        ref={activityListRef}
        className={activityListClassName}
        data-team-channel-scroll="true"
        onScroll={handleActivityScroll}
      >
        <div className={TEAM_TASK_ACTIVITY_SHELL_CLASS}>
          <div className={TEAM_TASK_ACTIVITY_STACK_CLASS}>
          {hiddenWaterfallSpacerHeight > 0 && (
            <div
              aria-hidden="true"
              data-team-channel-top-spacer="true"
              style={{ height: hiddenWaterfallSpacerHeight }}
            />
          )}
          {visibleWaterfallItems.map((item) => {
            const state = liveStateByMemberId.get(item.fromActorId);
            const isHumanAuthor = isHumanMailboxActor(item.fromActorId, humanActorId);
            const authorLabel = resolveThreadAuthorLabel(
              item.fromActorId,
              humanActorId,
              liveStateByMemberId
            );
            const seenActorIds = seenByMessageId[item.sequence] ?? [];
            const shouldShowSeenMeta =
              isHumanMailboxActor(item.fromActorId, humanActorId) || seenActorIds.length > 0;
            const seenProgress = resolveSeenProgressState(
              seenActorIds,
              memberIds,
              item.fromActorId
            );
            return (
              <div
                key={item.key}
                className={`${resolveActivityItemClassName(item.fromActorId, humanActorId)}${
                  shouldShowSeenMeta ? " pb-7 pr-8" : ""
                }`}
                data-activity-author-kind={isHumanAuthor ? "human" : "agent"}
                data-team-channel-item="true"
              >
                <div className={TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS}>
                  <div className={TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS}>
                    <span className={TEAM_TASK_ACTIVITY_AUTHOR_CLASS}>{authorLabel}</span>
                  </div>
                  <span className={TEAM_TASK_ACTIVITY_TIME_CLASS}>{formatTs(item.createdAt)}</span>
                </div>
                <ThreadRichText
                  className={TEAM_TASK_ACTIVITY_BODY_CLASS}
                  text={item.text}
                  renderHtml={renderTeamMessageHtml}
                />
                {shouldShowSeenMeta && (
                  <div className={TEAM_TASK_ACTIVITY_SEEN_META_CLASS}>
                    <HoverCard
                      openDelay={120}
                      closeDelay={80}
                      position="top-end"
                      shadow="md"
                      radius="md"
                    >
                      <HoverCard.Target>
                        {seenProgress.totalCount === 0 && seenActorIds.length === 0 ? (
                          <button
                            type="button"
                            className={TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS}
                            aria-label="Pending delivery"
                            title="Pending delivery"
                          >
                            <span className={TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS} />
                          </button>
                        ) : (
                          <button
                            type="button"
                            className={TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS}
                            aria-label={`Seen by ${seenProgress.readCount} of ${seenProgress.totalCount} recipients`}
                            title={`Seen by ${seenProgress.readCount} of ${seenProgress.totalCount} recipients`}
                          >
                            <span
                              className={TEAM_TASK_ACTIVITY_SEEN_DIAL_CLASS}
                              role="progressbar"
                              aria-valuenow={seenProgress.readCount}
                              aria-valuemin={0}
                              aria-valuemax={seenProgress.totalCount}
                              style={
                                {
                                  "--value": seenProgress.progress,
                                  "--size": "1rem",
                                  "--thickness": "1rem",
                                  width: "var(--size)",
                                  height: "var(--size)",
                                  background: `conic-gradient(rgba(31,122,61,0.82) calc(var(--value) * 1%), rgba(55,53,47,0.12) 0)`,
                                } satisfies SeenDialStyle
                              }
                            />
                          </button>
                        )}
                      </HoverCard.Target>
                      <HoverCard.Dropdown className={TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS}>
                        {seenProgress.totalCount === 0 && seenActorIds.length === 0 ? (
                          <>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS}>Delivery</div>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS}>
                              Pending delivery
                            </div>
                          </>
                        ) : (
                          <>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS}>Read state</div>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS}>
                              {`${seenProgress.readCount} read · ${seenProgress.unreadCount} unread`}
                            </div>
                            {seenProgress.readActorIds.length > 0 && (
                              <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS}>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS}>Read</div>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                                  {seenProgress.readActorIds.map((actorId) => (
                                    <span
                                      key={`${item.key}-read-${actorId}`}
                                      className="rounded-full border border-ui-border bg-ui-surface px-2 py-0.5"
                                    >
                                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )}
                            {seenProgress.unreadActorIds.length > 0 && (
                              <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS}>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS}>Unread</div>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                                  {seenProgress.unreadActorIds.map((actorId) => (
                                    <span
                                      key={`${item.key}-unread-${actorId}`}
                                      className="rounded-full border border-dashed border-ui-border bg-transparent px-2 py-0.5"
                                    >
                                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )}
                          </>
                        )}
                      </HoverCard.Dropdown>
                    </HoverCard>
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
          {!messagesLoading && orderedMessages.length === 0 && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              No channel messages yet.
            </div>
          )}
          </div>
        </div>
      </div>

      <div className={TEAM_TASK_COMPOSER_PANEL_CLASS}>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {stickToBottom && orderedMessages.length >= TEAM_TASK_TOP_JUMP_MIN_MESSAGES && (
            <button
              type="button"
              className={TEAM_TASK_JUMP_BUTTON_CLASS}
              onClick={() => {
                setStickToBottom(false);
                scrollActivityToTop();
              }}
              title="Jump to top"
              aria-label="Jump to top"
            >
              Jump to top
            </button>
          )}
          {activityJumpState.showJump && (
          <div className="flex justify-end">
            <button
              type="button"
              className={TEAM_TASK_JUMP_BUTTON_CLASS}
              onClick={() => {
                setStickToBottom(true);
                scrollActivityToBottom();
              }}
              title="Jump to bottom"
              aria-label="Jump to bottom"
            >
              Jump to bottom
            </button>
          </div>
          )}
        </div>
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
            {`@name for direct replies · Ctrl/Cmd + Enter sends`}
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

export const TeamTaskPanel = React.memo(TeamTaskPanelImpl);
TeamTaskPanel.displayName = "TeamTaskPanel";
