import React from "react";
import {
  Badge,
  CompactButton,
  ConversationBubble,
  EmptyState,
  SurfaceCard,
  ToolbarRow,
} from "../../ui/primitives";
import { DeterministicAvatar } from "../../components/deterministic_avatar";
import { TeamThreadRichText } from "./team_thread_rich_text";
import { TeamMessageComposer } from "./team_message_composer";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  type MentionCandidate,
  normalizeRawMentionActorId,
  renderMarkdownWithMentions,
  resolveDisplayName,
  resolveMentionDraftQuery,
} from "./mailbox_helpers";
import {
  TEAM_THREAD_BODY_CLASS,
  TEAM_THREAD_CHANNEL_BADGE_CLASS,
  TEAM_THREAD_COMPOSER_REGION_CLASS,
  TEAM_THREAD_EMPTY_REPLIES_CLASS,
  TEAM_THREAD_EMPTY_STATE_CLASS,
  TEAM_THREAD_HEADER_ACTIONS_CLASS,
  TEAM_THREAD_HEADER_BUTTON_CLASS,
  TEAM_THREAD_HEADER_CLASS,
  TEAM_THREAD_HEADER_CONTENT_CLASS,
  TEAM_THREAD_HEADER_ICON_CLASS,
  TEAM_THREAD_HEADER_TITLE_CLASS,
  TEAM_THREAD_HEADER_TITLE_ROW_CLASS,
  TEAM_THREAD_HELP_TEXT_CLASS,
  TEAM_THREAD_MESSAGE_AUTHOR_CLASS,
  TEAM_THREAD_MESSAGE_AVATAR_CLASS,
  TEAM_THREAD_MESSAGE_BUBBLE_CLASS,
  TEAM_THREAD_MESSAGE_CONTENT_CLASS,
  TEAM_THREAD_MESSAGE_META_ROW_CLASS,
  TEAM_THREAD_MESSAGE_ROW_CLASS,
  TEAM_THREAD_MISSING_TEXT_CLASS,
  TEAM_THREAD_ORIGINAL_BADGE_CLASS,
  TEAM_THREAD_PANE_CLASS,
  TEAM_THREAD_REPLY_COUNT_CLASS,
  TEAM_THREAD_REPLIES_LIST_CLASS,
  TEAM_THREAD_REPLIES_WINDOW_BUTTON_CLASS,
  TEAM_THREAD_REPLIES_WINDOW_NOTICE_CLASS,
  TEAM_THREAD_RICH_TEXT_CLASS,
  TEAM_THREAD_SECTION_ROW_CLASS,
  TEAM_THREAD_SECTION_TITLE_CLASS,
  TEAM_THREAD_SOURCE_CARD_CLASS,
  TEAM_THREAD_SOURCE_LABEL_CLASS,
  TEAM_THREAD_SOURCE_PREVIEW_CLASS,
  TEAM_THREAD_SOURCE_ROW_CLASS,
  TEAM_THREAD_SOURCE_VALUE_CLASS,
  TEAM_THREAD_TEXTAREA_CLASS,
  TEAM_THREAD_TRANSCRIPT_CLASS,
} from "../../ui/tailwind_classes";
import { isMobileInputViewport } from "../../components/input_dock";
import {
  DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE,
  shouldDefaultTeamConversationStickToBottom,
  windowTeamConversation,
} from "./team_conversation_viewport";

type TeamThreadReplyItem = {
  messageId: number;
  authorLabel: string | null;
  createdAt?: number | null;
  text: string;
};

export type TeamThreadRenderMessage = {
  messageId: number;
  authorLabel: string;
  createdAtLabel: string;
  text: string;
};

export type TeamThreadMessageRowProps = {
  message: TeamThreadRenderMessage;
  avatarStableId: string | number | null;
  renderSanitizedHtml: (text: string) => string;
  original?: boolean;
  showMissingTextWhenEmpty?: boolean;
};

type TeamThreadPaneProps = {
  channelLabel: string;
  rootMessageId: number | null;
  rootAuthorLabel: string | null;
  rootCreatedAt?: number | null;
  rootText: string | null;
  replies: TeamThreadReplyItem[];
  replyDraft: string;
  onReplyDraftChange: (value: string) => void;
  onSendReply: (payload: { text: string; mentionActorIds: string[] }) => void | Promise<void>;
  replyBusy?: boolean;
  formatTs: (ts?: number | null) => string;
  onViewInChannel: () => void;
  onClose: () => void;
  mentionCandidates?: MentionCandidate[];
  displayNameByActorId?: Record<string, string>;
};

function formatThreadReplyCount(count: number): string {
  return count === 1 ? "1 reply" : `${count} replies`;
}

export function areTeamThreadMessageRowPropsEqual(
  prev: TeamThreadMessageRowProps,
  next: TeamThreadMessageRowProps
): boolean {
  return (
    prev.avatarStableId === next.avatarStableId &&
    prev.renderSanitizedHtml === next.renderSanitizedHtml &&
    prev.original === next.original &&
    prev.showMissingTextWhenEmpty === next.showMissingTextWhenEmpty &&
    prev.message.messageId === next.message.messageId &&
    prev.message.authorLabel === next.message.authorLabel &&
    prev.message.createdAtLabel === next.message.createdAtLabel &&
    prev.message.text === next.message.text
  );
}

const TeamThreadMessageRow = React.memo(function TeamThreadMessageRow({
  message,
  avatarStableId,
  renderSanitizedHtml,
  original = false,
  showMissingTextWhenEmpty = false,
}: TeamThreadMessageRowProps) {
  return (
    <div className={TEAM_THREAD_MESSAGE_ROW_CLASS}>
      <DeterministicAvatar
        name={message.authorLabel}
        stableId={avatarStableId}
        className={TEAM_THREAD_MESSAGE_AVATAR_CLASS}
      />
      <div className={TEAM_THREAD_MESSAGE_CONTENT_CLASS}>
        <div className={TEAM_THREAD_MESSAGE_META_ROW_CLASS}>
          <span className={TEAM_THREAD_MESSAGE_AUTHOR_CLASS}>
            {message.authorLabel}
          </span>
          <span>{message.createdAtLabel}</span>
          <span>{`#${message.messageId}`}</span>
          {original ? (
            <Badge
              tone="outline"
              shape="pill"
              className={TEAM_THREAD_ORIGINAL_BADGE_CLASS}
            >
              Original
            </Badge>
          ) : null}
        </div>
        <ConversationBubble className={TEAM_THREAD_MESSAGE_BUBBLE_CLASS}>
          {message.text ? (
            <TeamThreadRichText
              className={TEAM_THREAD_RICH_TEXT_CLASS}
              text={message.text}
              renderSanitizedHtml={renderSanitizedHtml}
            />
          ) : showMissingTextWhenEmpty ? (
            <div className={TEAM_THREAD_MISSING_TEXT_CLASS}>
              Original content is not available in chat text form.
            </div>
          ) : (
            <TeamThreadRichText
              className={TEAM_THREAD_RICH_TEXT_CLASS}
              text=""
              renderSanitizedHtml={renderSanitizedHtml}
            />
          )}
        </ConversationBubble>
      </div>
    </div>
  );
}, areTeamThreadMessageRowPropsEqual);

function formatThreadSourceSummary(
  rootAuthorLabel: string | null,
  rootMessageId: number | null
): string | null {
  if (rootMessageId == null) {
    return null;
  }
  const authorLabel = rootAuthorLabel?.trim() || "Unknown";
  return `From ${authorLabel} · #${rootMessageId}`;
}

function formatThreadPreviewMentionText(
  text: string,
  displayNameByActorId?: Record<string, string>
): string {
  return text
    .replace(/<at>\s*([A-Za-z0-9._:-]+)\s*<\/at>/gi, (_match, actorId: string) => {
      const label = resolveDisplayName(actorId, displayNameByActorId, actorId);
      return `@${label}`;
    })
    .replace(
      /(^|[\s([{'"])@([A-Za-z0-9._:-]+)/g,
      (_match, prefix: string, actorId: string) => {
        const normalizedActorId = normalizeRawMentionActorId(actorId);
        const suffix = actorId.slice(normalizedActorId.length);
        const label = resolveDisplayName(
          normalizedActorId,
          displayNameByActorId,
          normalizedActorId || actorId
        );
        return `${prefix}@${label}${suffix}`;
      }
    );
}

function formatThreadSourcePreview(
  rootText: string | null,
  displayNameByActorId?: Record<string, string>
): string | null {
  const normalized = rootText?.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return null;
  }
  const truncatedInput = normalized.length > 500 ? normalized.slice(0, 500) : normalized;
  const displayText = formatThreadPreviewMentionText(truncatedInput, displayNameByActorId);
  if (displayText.length <= 120) {
    return displayText;
  }
  return `${displayText.slice(0, 117).trimEnd()}...`;
}

export const TeamThreadPane = React.memo(function TeamThreadPane({
  channelLabel,
  rootMessageId,
  rootAuthorLabel,
  rootCreatedAt,
  rootText,
  replies,
  replyDraft,
  onReplyDraftChange,
  onSendReply,
  replyBusy = false,
  formatTs,
  onViewInChannel,
  onClose,
  mentionCandidates = [],
  displayNameByActorId,
}: TeamThreadPaneProps) {
  const hasSelectedRoot = rootMessageId != null;
  const replyTextareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const [activeMention, setActiveMention] = React.useState<ReturnType<typeof resolveMentionDraftQuery>>(null);
  const [activeMentionIndex, setActiveMentionIndex] = React.useState(0);
  const [showAllReplies, setShowAllReplies] = React.useState(false);
  const [sendOnEnter, setSendOnEnter] = React.useState(() =>
    typeof window === "undefined" ? true : !isMobileInputViewport(window.innerWidth)
  );
  const replyCountLabel = formatThreadReplyCount(replies.length);
  const sourceSummary = formatThreadSourceSummary(rootAuthorLabel, rootMessageId);
  const sourcePreview = formatThreadSourcePreview(rootText, displayNameByActorId);
  const renderThreadMessageHtml = React.useCallback(
    (text: string) => renderMarkdownWithMentions(text, displayNameByActorId),
    [displayNameByActorId]
  );
  // Keep transcript display data stable while the reply draft changes so
  // typing in the composer does not rebuild every visible thread row.
  const rootRenderMessage = React.useMemo<TeamThreadRenderMessage | null>(() => {
    if (rootMessageId == null) {
      return null;
    }
    const authorLabel = rootAuthorLabel?.trim() || "Unknown";
    return {
      messageId: rootMessageId,
      authorLabel,
      createdAtLabel: formatTs(rootCreatedAt),
      text: rootText ?? "",
    };
  }, [formatTs, rootAuthorLabel, rootCreatedAt, rootMessageId, rootText]);
  const replyRenderMessages = React.useMemo<TeamThreadRenderMessage[]>(
    () =>
      replies.map((reply) => {
        const authorLabel = reply.authorLabel?.trim() || "Unknown";
        return {
          messageId: reply.messageId,
          authorLabel,
          createdAtLabel: formatTs(reply.createdAt),
          text: reply.text,
        };
      }),
    [formatTs, replies]
  );
  const shouldWindowReplies =
    !showAllReplies && shouldDefaultTeamConversationStickToBottom(replyRenderMessages.length);
  const replyWindow = React.useMemo(
    () =>
      windowTeamConversation(
        replyRenderMessages,
        shouldWindowReplies,
        DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE
      ),
    [replyRenderMessages, shouldWindowReplies]
  );
  const hiddenReplyCount = replyWindow.offset;
  React.useEffect(() => {
    setShowAllReplies(false);
  }, [rootMessageId]);
  React.useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    const syncViewport = () => {
      setSendOnEnter(!isMobileInputViewport(window.innerWidth));
    };
    syncViewport();
    window.addEventListener("resize", syncViewport);
    return () => {
      window.removeEventListener("resize", syncViewport);
    };
  }, []);
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
  const mentionQueryMatches = React.useCallback(
    (
      left: ReturnType<typeof resolveMentionDraftQuery>,
      right: ReturnType<typeof resolveMentionDraftQuery>
    ) =>
      left?.start === right?.start &&
      left?.end === right?.end &&
      left?.keyword === right?.keyword,
    []
  );
  const updateMentionQuery = React.useCallback((draft: string, cursor: number | null) => {
    if (cursor === null || Number.isNaN(cursor)) {
      setActiveMention(null);
      setActiveMentionIndex(0);
      return;
    }
    const next = resolveMentionDraftQuery(draft, cursor);
    setActiveMention((prev) => {
      if (mentionQueryMatches(prev, next)) {
        return prev;
      }
      setActiveMentionIndex(0);
      return next;
    });
  }, [mentionQueryMatches]);
  React.useEffect(() => {
    const textarea = replyTextareaRef.current;
    const cursor =
      textarea && document.activeElement === textarea
        ? textarea.selectionStart
        : replyDraft.length;
    updateMentionQuery(replyDraft, cursor);
  }, [replyDraft, updateMentionQuery]);
  const applyMentionSelection = React.useCallback(
    (candidate: MentionCandidate) => {
      if (!activeMention) {
        return;
      }
      const applied = applyMentionAtTag(replyDraft, activeMention, candidate.label);
      onReplyDraftChange(applied.text);
      setActiveMention(null);
      setActiveMentionIndex(0);
      requestAnimationFrame(() => {
        const textarea = replyTextareaRef.current;
        if (!textarea) {
          return;
        }
        textarea.focus();
        textarea.setSelectionRange(applied.cursor, applied.cursor);
      });
    },
    [activeMention, onReplyDraftChange, replyDraft]
  );
  const sendCurrentReply = React.useCallback(() => {
    const normalizedDraft = canonicalizeMentionDraft(replyDraft, mentionCandidates);
    if (!normalizedDraft.text.trim()) {
      return;
    }
    void onSendReply({
      text: normalizedDraft.text,
      mentionActorIds: normalizedDraft.mentionActorIds,
    });
  }, [mentionCandidates, onSendReply, replyDraft]);
  return (
    <SurfaceCard
      className={TEAM_THREAD_PANE_CLASS}
      data-team-surface="thread-pane"
    >
      <ToolbarRow className={TEAM_THREAD_HEADER_CLASS}>
        <div className={TEAM_THREAD_HEADER_CONTENT_CLASS}>
          <div className={TEAM_THREAD_HEADER_TITLE_ROW_CLASS}>
            <div className={TEAM_THREAD_HEADER_TITLE_CLASS}>
              Reply in thread
            </div>
            <Badge
              tone="outline"
              shape="pill"
              className={TEAM_THREAD_CHANNEL_BADGE_CLASS}
            >
              {channelLabel}
            </Badge>
            <span className={TEAM_THREAD_REPLY_COUNT_CLASS}>{replyCountLabel}</span>
          </div>
          <div className={TEAM_THREAD_HELP_TEXT_CLASS}>
            Keep the root summary-first. Use the thread for detailed context, logs, and follow-up.
          </div>
          {sourceSummary ? (
            <div className={TEAM_THREAD_SOURCE_CARD_CLASS}>
              <div className={TEAM_THREAD_SOURCE_ROW_CLASS}>
                <span className={TEAM_THREAD_SOURCE_LABEL_CLASS}>
                  Source
                </span>
                <span className={TEAM_THREAD_SOURCE_VALUE_CLASS}>{sourceSummary}</span>
              </div>
              {sourcePreview ? (
                <div
                  className={TEAM_THREAD_SOURCE_PREVIEW_CLASS}
                  data-team-surface="thread-source-preview"
                >
                  {sourcePreview}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
        <div className={TEAM_THREAD_HEADER_ACTIONS_CLASS}>
          <CompactButton
            type="button"
            className={TEAM_THREAD_HEADER_BUTTON_CLASS}
            onClick={onViewInChannel}
          >
            <i className={`bi bi-arrow-left ${TEAM_THREAD_HEADER_ICON_CLASS}`} aria-hidden="true" />
            View in channel
          </CompactButton>
          <CompactButton
            type="button"
            className={TEAM_THREAD_HEADER_BUTTON_CLASS}
            onClick={onClose}
          >
            <i className={`bi bi-x-lg ${TEAM_THREAD_HEADER_ICON_CLASS}`} aria-hidden="true" />
            Close thread
          </CompactButton>
        </div>
      </ToolbarRow>

      <div className={TEAM_THREAD_BODY_CLASS} data-team-surface="thread-scroll">
        {!hasSelectedRoot ? (
          <EmptyState
            title="Select a channel message"
            className={TEAM_THREAD_EMPTY_STATE_CLASS}
          >
            Thread roots open from existing channel messages.
          </EmptyState>
        ) : (
          <div className={TEAM_THREAD_TRANSCRIPT_CLASS}>
            {rootRenderMessage ? (
              <TeamThreadMessageRow
                message={rootRenderMessage}
                avatarStableId={rootAuthorLabel || rootMessageId}
                renderSanitizedHtml={renderThreadMessageHtml}
                original
                showMissingTextWhenEmpty
              />
            ) : null}
            <div className={TEAM_THREAD_SECTION_ROW_CLASS}>
              <div className={TEAM_THREAD_SECTION_TITLE_CLASS}>
                Replies
              </div>
              <span className={TEAM_THREAD_REPLY_COUNT_CLASS}>{replyCountLabel}</span>
            </div>
            <div className={TEAM_THREAD_REPLIES_LIST_CLASS}>
              {replies.length === 0 ? (
                <div className={TEAM_THREAD_EMPTY_REPLIES_CLASS}>
                  No replies yet.
                </div>
              ) : (
                <>
                  {hiddenReplyCount > 0 ? (
                    <div
                      className={TEAM_THREAD_REPLIES_WINDOW_NOTICE_CLASS}
                      data-team-surface="thread-replies-window-notice"
                    >
                      <span>
                        {`Showing latest ${replyWindow.items.length} of ${replyWindow.total} replies.`}
                      </span>
                      <CompactButton
                        type="button"
                        className={TEAM_THREAD_REPLIES_WINDOW_BUTTON_CLASS}
                        onClick={() => setShowAllReplies(true)}
                      >
                        Show earlier replies
                      </CompactButton>
                    </div>
                  ) : null}
                  {replyWindow.items.map((reply) => (
                    <TeamThreadMessageRow
                      key={reply.messageId}
                      message={reply}
                      avatarStableId={
                        reply.authorLabel === "Unknown" ? reply.messageId : reply.authorLabel
                      }
                      renderSanitizedHtml={renderThreadMessageHtml}
                    />
                  ))}
                </>
              )}
            </div>
          </div>
        )}
      </div>
      {hasSelectedRoot ? (
        <div className={TEAM_THREAD_COMPOSER_REGION_CLASS}>
          <TeamMessageComposer
            textareaRef={replyTextareaRef}
            draft={replyDraft}
            rows={2}
            placeholder={`Reply in thread · ${channelLabel}`}
            contextText={`Full context in thread · root message stays summary-first · ${channelLabel}`}
            helperText={
              sendOnEnter
                ? "@name to reply · Enter to reply"
                : "@name to reply · Enter adds a new line"
            }
            sendLabel="Reply"
            busyLabel="Replying..."
            busy={replyBusy}
            disabled={replyBusy || replyDraft.trim().length === 0}
            textareaClassName={TEAM_THREAD_TEXTAREA_CLASS}
            mentionOptions={activeMention ? filteredMentionCandidates : []}
            activeMentionIndex={activeMentionIndex}
            onSelectMention={applyMentionSelection}
            onDraftChange={(event) => {
              const nextDraft = event.currentTarget.value;
              onReplyDraftChange(nextDraft);
              updateMentionQuery(nextDraft, event.currentTarget.selectionStart);
            }}
            onTextareaClick={(event) =>
              updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
            }
            onTextareaKeyUp={(event) =>
              updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
            }
            onTextareaBlur={() => {
              setTimeout(() => {
                setActiveMention(null);
                setActiveMentionIndex(0);
              }, 0);
            }}
            onTextareaKeyDown={(event) => {
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
              if (
                event.key === "Enter" &&
                !event.shiftKey &&
                !event.altKey &&
                !event.metaKey &&
                !event.ctrlKey &&
                sendOnEnter &&
                !replyBusy &&
                replyDraft.trim().length > 0
              ) {
                event.preventDefault();
                sendCurrentReply();
              }
            }}
            onSend={sendCurrentReply}
          />
        </div>
      ) : null}
    </SurfaceCard>
  );
});
