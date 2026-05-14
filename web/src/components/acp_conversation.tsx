import React from "react";
import { ConversationItem } from "../conversation";
import { ACP_CONVERSATION_TOP_HINT_CLASS } from "../ui/tailwind_classes";
import {
  renderThreadMarkdownCached,
  resetThreadMarkdownCache,
} from "./thread_rich_text";
import {
  resetToolContentCaches,
} from "./acp_tool_content";
import {
  getAcpConversationCacheStats,
  type AcpConversationCacheStats,
} from "./acp_conversation_cache_stats";
import {
  AcpConversationItemRow,
  getConversationItemKey,
} from "./acp_conversation_items";
import { useAcpMarkdownRenderVersion } from "./use_acp_markdown_assets";
export { parseAnsiSegmentsCached } from "./acp_tool_content";
export { shouldAutoCollapseConversationItem } from "./acp_conversation_bubble";
export {
  deriveToolCallOpenState,
  isToolCallEffectivelyLive,
  shouldCollapseToolFoldWhenOutOfView,
} from "./acp_tool_fold";

type AcpConversationProps = {
  items: ConversationItem[];
  windowOffset: number;
  order?: "oldest_first" | "newest_first";
  isFrozenView: boolean;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  toolCallsDefaultCollapsed?: boolean;
  runStatus?: string | null;
  virtualTopSpacer: number;
  virtualBottomSpacer: number;
  stickToBottom: boolean;
  bottomAlignLatest?: boolean;
  pendingCount: number;
  avgHeight: number;
  topHint?: string | null;
  focusedToolCallId?: string | null;
  bottomClearancePx?: number;
  onScroll: () => void;
  onWheel?: (event: React.WheelEvent<HTMLDivElement>) => void;
  containerRef: React.Ref<HTMLDivElement>;
  ansi: (input: string) => string;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export function resetAcpConversationCaches(): void {
  resetThreadMarkdownCache();
  resetToolContentCaches();
}
export { getAcpConversationCacheStats };
export type { AcpConversationCacheStats };

export function renderMarkdownCached(text: string): string {
  return renderThreadMarkdownCached(text);
}

export function AcpConversation({
  items,
  windowOffset,
  order = "oldest_first",
  isFrozenView,
  shouldAutoCollapse,
  collapseCutoff,
  toolCallsDefaultCollapsed = false,
  runStatus,
  virtualTopSpacer,
  virtualBottomSpacer,
  stickToBottom,
  bottomAlignLatest = false,
  pendingCount,
  avgHeight,
  topHint,
  focusedToolCallId,
  bottomClearancePx = 0,
  onScroll,
  onWheel,
  containerRef,
  ansi,
  onSubmitRequestUserInput,
}: AcpConversationProps) {
  const markdownRenderVersion = useAcpMarkdownRenderVersion();
  const displayItems = React.useMemo(
    () =>
      items.map((msg, idx) => ({ msg, idx })).sort((left, right) =>
        order === "newest_first" ? right.idx - left.idx : left.idx - right.idx
      ),
    [items, order]
  );

  const bottomClearance = Number.isFinite(bottomClearancePx)
    ? Math.max(0, Math.round(bottomClearancePx))
    : 0;
  const conversationScrollStyle =
    bottomClearance > 0
      ? ({
          scrollPaddingBottom: `${bottomClearance}px`,
        } satisfies React.CSSProperties)
      : undefined;
  return (
    <div
      className="acp-conversation min-h-0 flex-1 overflow-auto px-0 py-0.5"
      data-acp-conversation-scroll="true"
      ref={containerRef}
      onScroll={onScroll}
      onWheel={onWheel}
      style={conversationScrollStyle}
    >
      <div
        className={`acp-conversation-inner flex w-full flex-col gap-1 ${
          bottomAlignLatest ? "min-h-full justify-end" : ""
        }`}
      >
        {topHint ? (
          <div className={ACP_CONVERSATION_TOP_HINT_CLASS}>
            {topHint}
          </div>
        ) : null}
        {virtualTopSpacer > 0 && (
          <div
            className="acp-conversation-spacer virtual-top"
            style={{ height: virtualTopSpacer }}
          />
        )}
        {displayItems.map(({ msg, idx }) => {
          const globalIndex = windowOffset + idx;
          const latestVisibleGlobalIndex = windowOffset + items.length - 1;
          return (
            <AcpConversationItemRow
              key={getConversationItemKey(msg, globalIndex)}
              msg={msg}
              globalIndex={globalIndex}
              latestVisibleGlobalIndex={latestVisibleGlobalIndex}
              focusedToolCallId={focusedToolCallId}
              shouldAutoCollapse={shouldAutoCollapse}
              collapseCutoff={collapseCutoff}
              toolCallsDefaultCollapsed={toolCallsDefaultCollapsed}
              isFrozenView={isFrozenView}
              runStatus={runStatus}
              ansi={ansi}
              markdownRenderVersion={markdownRenderVersion}
              onSubmitRequestUserInput={onSubmitRequestUserInput}
            />
          );
        })}
        {virtualBottomSpacer > 0 && (
          <div
            className="acp-conversation-spacer virtual-bottom"
            style={{ height: virtualBottomSpacer }}
          />
        )}
        {!stickToBottom && pendingCount > 0 && (
          <div
            className="acp-conversation-spacer"
            style={{ height: Math.round(pendingCount * avgHeight) }}
          />
        )}
      </div>
    </div>
  );
}

export type { AcpConversationProps };
