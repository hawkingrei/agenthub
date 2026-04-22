import React from "react";
import { CompactButton, EmptyState, SurfaceCard } from "../../ui/primitives";
import { TeamThreadRichText } from "./team_thread_rich_text";

type TeamThreadPaneProps = {
  channelLabel: string;
  rootMessageId: number | null;
  rootAuthorLabel: string | null;
  rootCreatedAt?: number | null;
  rootText: string | null;
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
  formatTs,
  onViewInChannel,
  onClose,
}: TeamThreadPaneProps) {
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
        {rootMessageId == null || !rootText ? (
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
              <TeamThreadRichText className="text-[13px] leading-6 text-notion-text" text={rootText} />
            </div>
            <div className="pt-1 text-[10px] leading-5 text-notion-text-muted">
              Replies stay scoped to this thread.
            </div>
          </div>
        )}
      </div>
    </SurfaceCard>
  );
});
