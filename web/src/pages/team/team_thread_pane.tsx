import React from "react";
import { ActionButton, CompactButton, EmptyState, SurfaceCard } from "../../ui/primitives";
import { TeamThreadRichText } from "./team_thread_rich_text";
import { TEAM_PANEL_TEXTAREA_CLASS } from "../../ui/tailwind_classes";

type TeamThreadReplyItem = {
  messageId: number;
  authorLabel: string | null;
  createdAt?: number | null;
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
  onSendReply: () => void | Promise<void>;
  replyBusy?: boolean;
  formatTs: (ts?: number | null) => string;
  onViewInChannel: () => void;
  onClose: () => void;
};

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
}: TeamThreadPaneProps) {
  const hasSelectedRoot = rootMessageId != null;
  return (
    <SurfaceCard
      className="flex min-h-0 w-full max-w-[340px] shrink-0 flex-col overflow-hidden border-notion-border/80"
      data-team-surface="thread-pane"
    >
      <div className="flex items-start justify-between gap-2 border-b border-notion-border/70 px-2.5 py-2">
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-semibold tracking-tight text-notion-text">Thread</div>
          <div className="mt-0.25 text-[11px] text-notion-text-muted">
            <span>— </span>
            <span>{channelLabel}</span>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <CompactButton
            type="button"
            className="px-1.5 text-[10px]"
            onClick={onViewInChannel}
          >
            <i className="bi bi-arrow-left text-[10px]" aria-hidden="true" />
            View in channel
          </CompactButton>
          <CompactButton type="button" className="px-1.5 text-[10px]" onClick={onClose}>
            <i className="bi bi-x-lg text-[10px]" aria-hidden="true" />
            Close thread
          </CompactButton>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2.5 py-2">
        {!hasSelectedRoot ? (
          <EmptyState
            title="Select a channel message"
            className="border-0 bg-transparent px-0 py-0"
          >
            Thread roots open from existing channel messages.
          </EmptyState>
        ) : (
          <div className="group relative flex flex-col gap-1.5 rounded-lg border-2 border-transparent px-2 py-2 transition hover:border-black hover:bg-white active:border-black active:bg-white">
            <div className="text-[10px] font-medium uppercase tracking-[0.06em] text-notion-text-muted/70">
              Original message
            </div>
            <div className="flex items-center gap-1.5 text-[10px] text-notion-text-muted">
              <span className="font-semibold text-notion-text">{rootAuthorLabel ?? "Unknown"}</span>
              <span>{formatTs(rootCreatedAt)}</span>
              <span>{`#${rootMessageId}`}</span>
            </div>
            <div className="rounded-[12px] border border-transparent bg-white/88 px-2 py-[5px]">
              {rootText ? (
                <TeamThreadRichText
                  className="text-[13px] leading-6 text-notion-text"
                  text={rootText}
                />
              ) : (
                <div className="text-[12px] italic leading-5 text-notion-text-muted">
                  Original content is not available in chat text form.
                </div>
              )}
            </div>
            <div className="pt-1 text-[10px] leading-5 text-notion-text-muted">Replies stay scoped to this thread.</div>
            <div className="mt-3 flex flex-col gap-2 border-t border-notion-border/70 pt-3">
              {replies.length === 0 ? (
                <div className="text-[11px] leading-5 text-notion-text-muted">
                  No replies yet.
                </div>
              ) : (
                replies.map((reply) => (
                  <div
                    key={reply.messageId}
                    className="group relative flex flex-col gap-1.5 rounded-lg border-2 border-transparent px-2 py-2 transition hover:border-black hover:bg-white active:border-black active:bg-white"
                  >
                    <div className="flex items-center gap-1.5 text-[10px] text-notion-text-muted">
                      <span className="font-semibold text-notion-text">
                        {reply.authorLabel ?? "Unknown"}
                      </span>
                      <span>{formatTs(reply.createdAt)}</span>
                      <span>{`#${reply.messageId}`}</span>
                    </div>
                    <div className="rounded-[12px] border border-transparent bg-white/88 px-2 py-[5px]">
                      <TeamThreadRichText
                        className="text-[13px] leading-6 text-notion-text"
                        text={reply.text}
                      />
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>
      {hasSelectedRoot ? (
        <div className="border-t border-notion-border/70 px-2.5 py-2">
          <textarea
            className={`${TEAM_PANEL_TEXTAREA_CLASS} min-h-[40px] px-2.5 py-1.5 text-[13px] leading-5`}
            rows={2}
            placeholder={`Reply in ${channelLabel}`}
            value={replyDraft}
            onChange={(event) => onReplyDraftChange(event.currentTarget.value)}
          />
          <div className="mt-2 flex items-center justify-end">
            <ActionButton
              type="button"
              onClick={onSendReply}
              disabled={replyBusy || replyDraft.trim().length === 0}
            >
              {replyBusy ? "Replying..." : "Reply"}
            </ActionButton>
          </div>
        </div>
      ) : null}
    </SurfaceCard>
  );
});
