import React from "react";
import {
  ConversationItem,
  flattenExploreGroupToolCalls,
} from "../conversation";
import { ACP_CONVERSATION_TOP_HINT_CLASS } from "../ui/tailwind_classes";
import {
  preloadThreadMarkdownAssets,
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
  ExploreGroupBubble,
  ToolCallBubble,
  ToolCallGroupBubble,
} from "./acp_tool_bubbles";
import { MarkdownBubble } from "./bubbles/markdown_bubble";
import { PlanBubble } from "./bubbles/plan_bubble";
import { ThinkingBubble } from "./bubbles/thinking_bubble";
export { parseAnsiSegmentsCached } from "./acp_tool_content";
export {
  deriveToolCallOpenState,
  isToolCallEffectivelyLive,
  shouldCollapseToolFoldWhenOutOfView,
} from "./acp_tool_bubbles";

type AcpConversationProps = {
  items: ConversationItem[];
  windowOffset: number;
  isFrozenView: boolean;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  runStatus?: string | null;
  virtualTopSpacer: number;
  virtualBottomSpacer: number;
  stickToBottom: boolean;
  pendingCount: number;
  avgHeight: number;
  topHint?: string | null;
  focusedToolCallId?: string | null;
  bottomClearancePx?: number;
  onScroll: () => void;
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
  isFrozenView,
  shouldAutoCollapse,
  collapseCutoff,
  runStatus,
  virtualTopSpacer,
  virtualBottomSpacer,
  stickToBottom,
  pendingCount,
  avgHeight,
  topHint,
  focusedToolCallId,
  bottomClearancePx = 0,
  onScroll,
  containerRef,
  ansi,
  onSubmitRequestUserInput,
}: AcpConversationProps) {
  const [markdownRenderVersion, setMarkdownRenderVersion] = React.useState(0);

  React.useEffect(() => {
    let cancelled = false;
    void preloadThreadMarkdownAssets()
      .then(() => {
        if (!cancelled) {
          setMarkdownRenderVersion(1);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

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
      className="acp-conversation min-h-0 flex-1 overflow-auto px-0 py-1"
      data-acp-conversation-scroll="true"
      ref={containerRef}
      onScroll={onScroll}
      style={conversationScrollStyle}
    >
      <div className="acp-conversation-inner flex w-full flex-col gap-1.5">
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
        {items.map((msg, idx) => {
          const globalIndex = windowOffset + idx;
          const key = getConversationItemKey(msg, globalIndex);
          const isFocusedToolCall = isConversationItemFocusedToolCall(
            msg,
            focusedToolCallId ?? null
          );
          return (
            <div
              key={key}
              className={`acp-conversation-item${isFocusedToolCall ? " is-focused ring-2 ring-sky-300 ring-offset-2 ring-offset-white" : ""}`}
              data-conversation-item-key={key}
              data-tool-call-id={getConversationItemToolCallId(msg)}
            >
              <ConversationBubble
                msg={msg}
                globalIndex={globalIndex}
                shouldAutoCollapse={shouldAutoCollapse}
                collapseCutoff={collapseCutoff}
                isFrozenView={isFrozenView}
                runStatus={runStatus}
                ansi={ansi}
                markdownRenderVersion={markdownRenderVersion}
                onSubmitRequestUserInput={onSubmitRequestUserInput}
              />
            </div>
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

type ConversationBubbleProps = {
  msg: ConversationItem;
  globalIndex: number;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  isFrozenView: boolean;
  runStatus?: string | null;
  ansi: (input: string) => string;
  markdownRenderVersion: number;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

const ConversationBubble = React.memo(
  function ConversationBubble({
    msg,
    globalIndex,
    shouldAutoCollapse,
    collapseCutoff,
    isFrozenView,
    runStatus,
    ansi,
    markdownRenderVersion,
    onSubmitRequestUserInput,
  }: ConversationBubbleProps) {
    const autoCollapse =
      shouldAutoCollapse && !isFrozenView && globalIndex < collapseCutoff;

    if (msg.kind === "agent_thinking") {
      return <ThinkingBubble text={msg.text} live={msg.live} />;
    }

    if (msg.kind === "agent_plan") {
      return <PlanBubble msg={msg} autoCollapse={autoCollapse} />;
    }

    if (msg.kind === "explore_group") {
      return (
        <ExploreGroupBubble
          msg={msg}
          ansi={ansi}
          runStatus={runStatus}
          autoCollapse={autoCollapse}
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    }

    if (msg.kind === "tool_call") {
      return (
        <ToolCallBubble
          msg={msg}
          ansi={ansi}
          runStatus={runStatus}
          autoCollapse={autoCollapse}
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    }
    if (msg.kind === "tool_call_group") {
      return (
        <ToolCallGroupBubble
          msg={msg}
          ansi={ansi}
          runStatus={runStatus}
          autoCollapse={autoCollapse}
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    }

    if (msg.kind === "agent_message") {
      return (
        <MarkdownBubble
          className="agent_message"
          text={msg.text}
          markdownRenderVersion={markdownRenderVersion}
        />
      );
    }

    return (
      <MarkdownBubble
        className="user_message"
        text={msg.text}
        markdownRenderVersion={markdownRenderVersion}
      />
    );
  },
  areConversationBubblePropsEqual
);

function areConversationBubblePropsEqual(
  prev: Readonly<ConversationBubbleProps>,
  next: Readonly<ConversationBubbleProps>
): boolean {
  if (prev.msg !== next.msg) return false;
  if (prev.globalIndex !== next.globalIndex) return false;
  if (prev.shouldAutoCollapse !== next.shouldAutoCollapse) return false;
  if (prev.collapseCutoff !== next.collapseCutoff) return false;
  if (prev.isFrozenView !== next.isFrozenView) return false;
  if (prev.ansi !== next.ansi) return false;
  if (prev.markdownRenderVersion !== next.markdownRenderVersion) return false;
  if (prev.onSubmitRequestUserInput !== next.onSubmitRequestUserInput) return false;
  if (
    prev.msg.kind === "tool_call" ||
    prev.msg.kind === "tool_call_group" ||
    prev.msg.kind === "explore_group"
  ) {
    return prev.runStatus === next.runStatus;
  }
  return true;
}
function getConversationItemKey(msg: ConversationItem, fallback: number): string {
  if (msg.kind === "tool_call") return `tool_call:${msg.id}`;
  if (msg.kind === "tool_call_group") {
    const ids = msg.calls.map((call) => call.id).join(",");
    return `tool_call_group:${ids}`;
  }
  if (msg.kind === "explore_group") {
    const ids = flattenExploreGroupToolCalls(msg.items)
      .map((call) => call.id)
      .join(",");
    const fallbackKey = msg.event_id ?? msg.seq ?? fallback;
    return `explore_group:${ids || fallbackKey}`;
  }
  if (msg.event_id != null) return `${msg.kind}:event:${msg.event_id}`;
  if (msg.seq) return `${msg.kind}:seq:${msg.seq}`;
  return `${msg.kind}:idx:${fallback}`;
}

function getConversationItemToolCallId(msg: ConversationItem): string | undefined {
  if (msg.kind === "tool_call") return msg.id;
  return undefined;
}

function isConversationItemFocusedToolCall(
  msg: ConversationItem,
  focusedToolCallId: string | null
): boolean {
  if (!focusedToolCallId) return false;
  if (msg.kind === "tool_call") return msg.id === focusedToolCallId;
  if (msg.kind === "tool_call_group") {
    return msg.calls.some((call) => call.id === focusedToolCallId);
  }
  if (msg.kind === "explore_group") {
    return flattenExploreGroupToolCalls(msg.items).some(
      (call) => call.id === focusedToolCallId
    );
  }
  return false;
}
