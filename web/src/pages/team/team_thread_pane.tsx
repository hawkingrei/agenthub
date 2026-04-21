import React from "react";
import { ActionButton, EmptyState, SurfaceCard } from "../../ui/primitives";
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
      <div className="flex items-start justify-between gap-2.5 border-b border-notion-border/70 px-3 py-2.5">
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-semibold tracking-tight text-notion-text">Thread</div>
          <div className="mt-0.25 text-[12px] text-notion-text-muted">
            <span>— </span>
            <span>{channelLabel}</span>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <ActionButton
            type="button"
            tone="ghost"
            size="sm"
            className="px-2.5"
            onClick={onViewInChannel}
          >
            View in channel
          </ActionButton>
          <ActionButton type="button" tone="ghost" size="sm" className="px-2.5" onClick={onClose}>
            Close thread
          </ActionButton>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-3 py-2.5">
        {rootMessageId == null || !rootText ? (
          <EmptyState
            title="Select a channel message"
            className="border-0 bg-transparent px-0 py-0"
          >
            Thread roots open from existing channel messages.
          </EmptyState>
        ) : (
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center gap-1.5 text-[11px] text-notion-text-muted">
              <span className="font-semibold text-notion-text">{rootAuthorLabel ?? "Unknown"}</span>
              <span>{formatTs(rootCreatedAt)}</span>
              <span>{`#${rootMessageId}`}</span>
            </div>
            <div className="rounded-[16px] border border-notion-border bg-white px-2.5 py-2">
              <TeamThreadRichText className="text-[13px] leading-6 text-notion-text" text={rootText} />
            </div>
            <div className="pt-1.5 text-[11px] leading-5 text-notion-text-muted">
              Thread reply persistence is converging into the Team actor channel/thread contract.
            </div>
          </div>
        )}
      </div>
    </SurfaceCard>
  );
});
