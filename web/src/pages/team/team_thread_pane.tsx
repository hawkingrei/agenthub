import React from "react";
import {
  ActionButton,
  Badge,
  CompactButton,
  ConversationBubble,
  EmptyState,
  MenuOptionButton,
  SurfaceCard,
  ToolbarRow,
} from "../../ui/primitives";
import { DeterministicAvatar } from "../../components/deterministic_avatar";
import { TeamThreadRichText } from "./team_thread_rich_text";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  type MentionCandidate,
  resolveMentionDraftQuery,
} from "./mailbox_helpers";
import {
  CONVERSATION_MESSAGE_INLINE_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS,
  TEAM_MESSAGE_COMPOSER_SEND_BUTTON_CLASS,
  TEAM_MESSAGE_COMPOSER_SHELL_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
} from "../../ui/tailwind_classes";
import { isMobileInputViewport } from "../../components/input_dock";

type TeamThreadReplyItem = {
  messageId: number;
  authorLabel: string | null;
  createdAt?: number | null;
  text: string;
};

type TeamThreadRenderMessage = {
  messageId: number;
  authorLabel: string;
  createdAtLabel: string;
  text: string;
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
};

const TEAM_THREAD_MESSAGE_ROW_CLASS =
  `${CONVERSATION_MESSAGE_INLINE_ROW_CLASS} rounded-xl`;
const TEAM_THREAD_MESSAGE_AVATAR_CLASS =
  "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-black/8 bg-white text-[10px] font-semibold uppercase tracking-tight text-notion-text-muted shadow-sm";
const TEAM_THREAD_MESSAGE_META_ROW_CLASS =
  "flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[10px] text-notion-text-muted";
const TEAM_THREAD_MESSAGE_CONTENT_CLASS = "min-w-0 flex flex-1 flex-col gap-1";
const TEAM_THREAD_MESSAGE_BUBBLE_CLASS =
  "w-full rounded-[12px] border border-black/6 bg-white/96 px-2 py-[5px] shadow-none [&_.md-blockquote]:bg-slate-50/92 [&_.md-table-wrap]:bg-white/88 [&_.md-table-wrap]:border-black/6 [&_.md-code-block]:border-slate-900/80";
const TEAM_THREAD_SECTION_ROW_CLASS =
  "flex flex-wrap items-center justify-between gap-2 px-2";
function formatThreadReplyCount(count: number): string {
  return count === 1 ? "1 reply" : `${count} replies`;
}
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

function formatThreadSourcePreview(rootText: string | null): string | null {
  const normalized = rootText?.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return null;
  }
  if (normalized.length <= 120) {
    return normalized;
  }
  return `${normalized.slice(0, 117).trimEnd()}...`;
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
}: TeamThreadPaneProps) {
  const hasSelectedRoot = rootMessageId != null;
  const replyTextareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const [activeMention, setActiveMention] = React.useState<ReturnType<typeof resolveMentionDraftQuery>>(null);
  const [activeMentionIndex, setActiveMentionIndex] = React.useState(0);
  const [sendOnEnter, setSendOnEnter] = React.useState(() =>
    typeof window === "undefined" ? true : !isMobileInputViewport(window.innerWidth)
  );
  const replyCountLabel = formatThreadReplyCount(replies.length);
  const sourceSummary = formatThreadSourceSummary(rootAuthorLabel, rootMessageId);
  const sourcePreview = formatThreadSourcePreview(rootText);
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
      className="flex min-h-0 w-full max-w-[360px] shrink-0 flex-col overflow-hidden border-notion-border/80"
      data-team-surface="thread-pane"
    >
      <ToolbarRow className="items-start gap-2 border-b border-notion-border/70 px-2.5 py-2">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
              Reply in thread
            </div>
            <Badge
              tone="outline"
              shape="pill"
              className="border-black/8 bg-white/92 px-2 py-0 text-[9px] font-semibold text-notion-text-muted"
            >
              {channelLabel}
            </Badge>
            <span className="text-[10px] text-notion-text-muted">{replyCountLabel}</span>
          </div>
          <div className="mt-0.5 text-[11px] text-notion-text-muted">
            Keep the root summary-first. Use the thread for detailed context, logs, and follow-up.
          </div>
          {sourceSummary ? (
            <div className="mt-1 flex max-w-full flex-col gap-0.5 rounded-md border border-black/6 bg-white/92 px-2 py-1 text-[10px] font-medium text-notion-text-muted">
              <div className="inline-flex min-w-0 items-center gap-1.5">
                <span className="uppercase tracking-[0.08em] text-notion-text-muted/70">
                  Source
                </span>
                <span className="truncate text-notion-text">{sourceSummary}</span>
              </div>
              {sourcePreview ? (
                <div className="truncate text-[11px] font-normal leading-4 text-notion-text-muted/90">
                  {sourcePreview}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <CompactButton
            type="button"
            className="px-1.5 text-[10px] text-notion-text-muted/80"
            onClick={onViewInChannel}
          >
            <i className="bi bi-arrow-left text-[10px]" aria-hidden="true" />
            View in channel
          </CompactButton>
          <CompactButton
            type="button"
            className="px-1.5 text-[10px] text-notion-text-muted/80"
            onClick={onClose}
          >
            <i className="bi bi-x-lg text-[10px]" aria-hidden="true" />
            Close thread
          </CompactButton>
        </div>
      </ToolbarRow>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2.5 py-2">
        {!hasSelectedRoot ? (
          <EmptyState
            title="Select a channel message"
            className="border-0 bg-transparent px-0 py-0"
          >
            Thread roots open from existing channel messages.
          </EmptyState>
        ) : (
          <div className="flex flex-col gap-3">
            <div className={TEAM_THREAD_MESSAGE_ROW_CLASS}>
              <DeterministicAvatar
                name={rootRenderMessage?.authorLabel ?? "Unknown"}
                stableId={rootAuthorLabel || rootMessageId}
                className={TEAM_THREAD_MESSAGE_AVATAR_CLASS}
              />
              <div className={TEAM_THREAD_MESSAGE_CONTENT_CLASS}>
                <div className={TEAM_THREAD_MESSAGE_META_ROW_CLASS}>
                  <span className="font-semibold text-notion-text">
                    {rootRenderMessage?.authorLabel ?? "Unknown"}
                  </span>
                  <span>{rootRenderMessage?.createdAtLabel ?? ""}</span>
                  <span>{`#${rootRenderMessage?.messageId ?? rootMessageId}`}</span>
                  <Badge
                    tone="outline"
                    shape="pill"
                    className="ml-0.5 border-black/8 bg-white/92 px-2 py-0 text-[9px] font-semibold uppercase tracking-[0.06em] text-notion-text-muted"
                  >
                    Original
                  </Badge>
                </div>
                <ConversationBubble className={TEAM_THREAD_MESSAGE_BUBBLE_CLASS}>
                  {rootRenderMessage?.text ? (
                    <TeamThreadRichText
                      className="text-[13px] leading-6 text-notion-text"
                      text={rootRenderMessage.text}
                    />
                  ) : (
                    <div className="text-[12px] italic leading-5 text-notion-text-muted">
                      Original content is not available in chat text form.
                    </div>
                  )}
                </ConversationBubble>
              </div>
            </div>
            <div className={TEAM_THREAD_SECTION_ROW_CLASS}>
              <div className="text-[10px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                Replies
              </div>
              <span className="text-[10px] text-notion-text-muted">{replyCountLabel}</span>
            </div>
            <div className="flex flex-col gap-2 border-t border-notion-border/70 pt-3">
              {replies.length === 0 ? (
                <div className="px-2 text-[11px] leading-5 text-notion-text-muted">
                  No replies yet.
                </div>
              ) : (
                replyRenderMessages.map((reply) => (
                  <div key={reply.messageId} className={TEAM_THREAD_MESSAGE_ROW_CLASS}>
                    <DeterministicAvatar
                      name={reply.authorLabel}
                      stableId={reply.authorLabel === "Unknown" ? reply.messageId : reply.authorLabel}
                      className={TEAM_THREAD_MESSAGE_AVATAR_CLASS}
                    />
                    <div className={TEAM_THREAD_MESSAGE_CONTENT_CLASS}>
                      <div className={TEAM_THREAD_MESSAGE_META_ROW_CLASS}>
                        <span className="font-semibold text-notion-text">{reply.authorLabel}</span>
                        <span>{reply.createdAtLabel}</span>
                        <span>{`#${reply.messageId}`}</span>
                      </div>
                      <ConversationBubble className={TEAM_THREAD_MESSAGE_BUBBLE_CLASS}>
                        <TeamThreadRichText
                          className="text-[13px] leading-6 text-notion-text"
                          text={reply.text}
                        />
                      </ConversationBubble>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>
      {hasSelectedRoot ? (
        <div className="border-t border-notion-border/55 bg-white/92 px-2.5 py-1.5">
          <div className={TEAM_MESSAGE_COMPOSER_SHELL_CLASS}>
            <div className="px-1 pb-1 text-[10px] leading-5 text-notion-text-muted">
              Reply in thread · {channelLabel}
            </div>
            <div className={TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS}>
              <textarea
                ref={replyTextareaRef}
                className={`${TEAM_PANEL_TEXTAREA_CLASS} min-h-[40px] flex-1 border-transparent px-0 py-0 text-[13px] leading-5 shadow-none focus:border-transparent focus:ring-0`}
                rows={2}
                placeholder={`Reply in thread · ${channelLabel}`}
                value={replyDraft}
                onChange={(event) => {
                  const nextDraft = event.currentTarget.value;
                  onReplyDraftChange(nextDraft);
                  updateMentionQuery(nextDraft, event.currentTarget.selectionStart);
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
              />
              <ActionButton
                type="button"
                className={TEAM_MESSAGE_COMPOSER_SEND_BUTTON_CLASS}
                onClick={() => {
                  sendCurrentReply();
                }}
                disabled={replyBusy || replyDraft.trim().length === 0}
              >
                {replyBusy ? "Replying..." : "Reply"}
              </ActionButton>
            </div>
            {activeMention && filteredMentionCandidates.length > 0 && (
              <div className="mt-2 overflow-hidden rounded-xl border border-ui-border bg-ui-surface shadow-sm">
                <div className="px-3 py-1 text-xs text-ui-text-muted">
                  Select teammate mention (`@` without selection stays plain text)
                </div>
                <div className="max-h-44 overflow-auto py-1">
                  {filteredMentionCandidates.map((candidate, index) => (
                    <MenuOptionButton
                      key={candidate.actorId}
                      active={index === activeMentionIndex}
                      data-team-mention-option={candidate.actorId}
                      onMouseDown={(event) => {
                        event.preventDefault();
                        applyMentionSelection(candidate);
                      }}
                    >
                      <span>{candidate.label}</span>
                      <span className="text-[11px] text-ui-text-muted">{`@${candidate.label}`}</span>
                    </MenuOptionButton>
                  ))}
                </div>
              </div>
            )}
            <ToolbarRow className={TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS}>
              <span className={TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS}>
                {sendOnEnter
                  ? "@name to reply · Enter to reply"
                  : "@name to reply · Enter adds a new line"}
              </span>
            </ToolbarRow>
          </div>
        </div>
      ) : null}
    </SurfaceCard>
  );
});
