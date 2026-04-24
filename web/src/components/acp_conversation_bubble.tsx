import React from "react";
import { ConversationItem } from "../conversation";
import { MarkdownBubble } from "./bubbles/markdown_bubble";
import { PlanBubble } from "./bubbles/plan_bubble";
import { ThinkingBubble } from "./bubbles/thinking_bubble";

const LazyToolCallBubble = React.lazy(async () => {
  const mod = await import("./acp_tool_bubbles");
  return { default: mod.ToolCallBubble };
});

const LazyToolCallGroupBubble = React.lazy(async () => {
  const mod = await import("./acp_tool_bubbles");
  return { default: mod.ToolCallGroupBubble };
});

const LazyExploreGroupBubble = React.lazy(async () => {
  const mod = await import("./acp_tool_bubbles");
  return { default: mod.ExploreGroupBubble };
});

export type AcpConversationBubbleProps = {
  msg: ConversationItem;
  globalIndex: number;
  latestVisibleGlobalIndex: number;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  isFrozenView: boolean;
  runStatus?: string | null;
  ansi: (input: string) => string;
  markdownRenderVersion: number;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export function shouldAutoCollapseConversationItem(
  msg: ConversationItem,
  {
    globalIndex,
    latestVisibleGlobalIndex,
    shouldAutoCollapse,
    collapseCutoff,
    isFrozenView,
  }: {
    globalIndex: number;
    latestVisibleGlobalIndex: number;
    shouldAutoCollapse: boolean;
    collapseCutoff: number;
    isFrozenView: boolean;
  }
): boolean {
  if (isFrozenView) return false;
  const tailWindowAutoCollapse =
    shouldAutoCollapse && globalIndex < collapseCutoff;
  if (
    msg.kind === "tool_call" ||
    msg.kind === "tool_call_group" ||
    msg.kind === "explore_group"
  ) {
    return globalIndex < latestVisibleGlobalIndex || tailWindowAutoCollapse;
  }
  if (msg.kind === "agent_plan") {
    return tailWindowAutoCollapse;
  }
  return false;
}

export const AcpConversationBubble = React.memo(
  function AcpConversationBubble({
    msg,
    globalIndex,
    latestVisibleGlobalIndex,
    shouldAutoCollapse,
    collapseCutoff,
    isFrozenView,
    runStatus,
    ansi,
    markdownRenderVersion,
    onSubmitRequestUserInput,
  }: AcpConversationBubbleProps) {
    const autoCollapse = shouldAutoCollapseConversationItem(msg, {
      globalIndex,
      latestVisibleGlobalIndex,
      shouldAutoCollapse,
      collapseCutoff,
      isFrozenView,
    });

    if (msg.kind === "agent_thinking") {
      return <ThinkingBubble text={msg.text} live={msg.live} />;
    }

    if (msg.kind === "agent_plan") {
      return <PlanBubble msg={msg} autoCollapse={autoCollapse} />;
    }

    if (msg.kind === "explore_group") {
      return (
        <React.Suspense fallback={<DeferredToolBubblePlaceholder label="Explore" />}>
          <LazyExploreGroupBubble
            msg={msg}
            ansi={ansi}
            runStatus={runStatus}
            autoCollapse={autoCollapse}
            onSubmitRequestUserInput={onSubmitRequestUserInput}
          />
        </React.Suspense>
      );
    }

    if (msg.kind === "tool_call") {
      return (
        <React.Suspense fallback={<DeferredToolBubblePlaceholder label="Tool Call" />}>
          <LazyToolCallBubble
            msg={msg}
            ansi={ansi}
            runStatus={runStatus}
            autoCollapse={autoCollapse}
            onSubmitRequestUserInput={onSubmitRequestUserInput}
          />
        </React.Suspense>
      );
    }

    if (msg.kind === "tool_call_group") {
      return (
        <React.Suspense fallback={<DeferredToolBubblePlaceholder label="Tool Calls" />}>
          <LazyToolCallGroupBubble
            msg={msg}
            ansi={ansi}
            runStatus={runStatus}
            autoCollapse={autoCollapse}
            onSubmitRequestUserInput={onSubmitRequestUserInput}
          />
        </React.Suspense>
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
  prev: Readonly<AcpConversationBubbleProps>,
  next: Readonly<AcpConversationBubbleProps>
): boolean {
  if (prev.msg !== next.msg) return false;
  if (prev.globalIndex !== next.globalIndex) return false;
  if (prev.latestVisibleGlobalIndex !== next.latestVisibleGlobalIndex) return false;
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

function DeferredToolBubblePlaceholder({ label }: { label: string }) {
  return (
    <div className="acp-row group relative mb-1 flex w-full flex-col items-start rounded-lg border-2 border-transparent px-2 py-2">
      <div className="w-full max-w-full rounded-[10px] border border-black/6 bg-white/96 px-2 py-1.5 text-[13px] font-semibold text-slate-900 shadow-none">
        {label}
      </div>
    </div>
  );
}
