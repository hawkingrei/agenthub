import React from "react";
import type { AcpTerminalActivity } from "../acp";
import {
  ConversationItem,
  ExploreGroupConversationItem,
  ToolCallConversationItem,
  ToolCallGroupConversationItem,
  flattenExploreGroupToolCalls,
  formatConversationPreview,
  isToolCallLive,
  unescapeLineBreaks,
} from "../conversation";
import {
  ACP_CONVERSATION_TOP_HINT_CLASS,
  ACP_DIFF_PRE_CLASS,
  ACP_SEGMENTED_BUTTON_CLASS,
  ACP_SEGMENTED_NOTE_WARNING_CLASS,
  ACP_TERMINAL_PRE_CLASS,
  ACP_TOOL_STATUS_CLASS,
  ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS,
  ACP_PAYLOAD_MARKDOWN_CLASS,
} from "../ui/tailwind_classes";
import {
  REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
  buildRequestUserInputSubmissionText,
  countAnsweredRequestUserInputQuestions,
  createInitialRequestUserInputDrafts,
  formatRequestUserInputSummary,
  parseRequestUserInputQuestions,
  parseRequestUserInputResponse,
  splitRequestUserInputAnswer,
  type RequestUserInputDrafts,
  type RequestUserInputQuestion,
  type RequestUserInputResponse,
} from "../request_user_input";
import {
  getThreadMarkdownCacheStats,
  preloadThreadMarkdownAssets,
  renderThreadMarkdownCached,
  resetThreadMarkdownCache,
  ThreadRichText,
} from "./thread_rich_text";
import {
  extractToolCallDetails,
  formatToolCallDurationLabel,
  resolveToolGroupStatusClassName,
  selectToolCallOutputForDisplay,
  type ToolCallDetailItem,
  type ToolGroupStatusTone,
} from "./acp_tool_call_meta";
import { MarkdownBubble } from "./bubbles/markdown_bubble";
import { PlanBubble } from "./bubbles/plan_bubble";
import { ThinkingBubble } from "./bubbles/thinking_bubble";

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

const ANSI_SEGMENT_CACHE_LIMIT = 512;
const ANSI_SEGMENT_CACHE_MAX_BYTES = 4 * 1024 * 1024;
const ANSI_SEGMENT_CACHE_MAX_ENTRY_CHARS = 120_000;
const TOOL_PAYLOAD_PREVIEW_LIMIT = 64;
const TOOL_PAYLOAD_MAX_NESTED_DEPTH = 2;
const TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED = new Set<string>([
  "turnid",
  "processid",
  "source",
  "callid",
  "cwd",
  "success",
  "duration",
  "durationms",
  "elapsedms",
  "latencyms",
]);
const TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED = [
  "aggregatedoutput",
  "formattedoutput",
  "stdout",
] as const;
const TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED = new Set<string>(
  TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED
);
const TOOL_PAYLOAD_HIDE_EMPTY_STREAM_KEY_NORMALIZED = new Set<string>([
  "stdout",
  "stderr",
]);
const TOOL_PAYLOAD_PLAIN_TEXT_KEY_NORMALIZED = new Set<string>([
  "parsedcmd",
  "context",
  "content",
]);
const TOOL_TEXT_INITIAL_LINES = 36;
const TOOL_TEXT_LINE_CHUNK = 120;
const TOOL_TEXT_MARKDOWN_FALLBACK_LINES = 260;
const TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH = 16000;
const TOOL_PAYLOAD_INITIAL_ITEMS = 8;
const TOOL_PAYLOAD_ITEM_CHUNK = 16;
const TOOL_VISIBILITY_COLLAPSE_THRESHOLD = 0;
const ACP_SUBFOLD_CLASS = "acp-subfold mt-1.5";
const ACP_PAYLOAD_CARD_CLASS =
  "acp-payload-card overflow-hidden rounded-[10px] border border-[#dde2db] bg-[#fcfcfa] px-[7px] py-1.5 shadow-[0_1px_0_rgba(15,23,42,0.03)] max-[720px]:rounded-lg max-[720px]:px-1.5 max-[720px]:py-[5px]";
const ACP_PAYLOAD_GRID_CLASS = "acp-payload-grid m-0 grid gap-1.5";
const ACP_PAYLOAD_ROW_CLASS =
  "acp-payload-row grid grid-cols-[minmax(84px,128px)_minmax(0,1fr)] items-start gap-1.5 rounded-md border border-slate-200 bg-slate-50 px-2 py-1.5 [&>dt]:break-words [&>dt]:font-mono [&>dt]:text-[11px] [&>dt]:leading-[1.4] [&>dt]:text-slate-500 [&>dd]:m-0 [&>dd]:min-w-0 max-[720px]:grid-cols-[minmax(64px,96px)_minmax(0,1fr)] max-[720px]:gap-[5px] max-[600px]:grid-cols-1 max-[600px]:gap-1";
const ACP_PAYLOAD_SCALAR_CLASS =
  "acp-payload-scalar inline-block max-w-full break-words font-mono text-[11px] leading-[1.45] text-slate-800";
const ACP_PAYLOAD_SCALAR_MUTED_CLASS =
  "acp-payload-scalar muted inline-block max-w-full break-words font-mono text-[11px] leading-[1.45] text-slate-400";
const ACP_PAYLOAD_TEXT_BASE_CLASS =
  "acp-content acp-payload-text m-0 whitespace-pre-wrap font-mono text-[11px] leading-[1.45] text-slate-800";
const ACP_PAYLOAD_TEXT_ASCII_CLASS =
  `${ACP_PAYLOAD_TEXT_BASE_CLASS} acp-payload-ascii overflow-x-auto whitespace-pre`;
const ACP_CONTENT_TEXT_BASE_CLASS =
  `${ACP_TERMINAL_PRE_CLASS} acp-content acp-payload-text`;
const ACP_CONTENT_TEXT_ASCII_CLASS =
  `${ACP_CONTENT_TEXT_BASE_CLASS} acp-payload-ascii whitespace-pre`;
const ACP_CONTENT_MARKDOWN_CLASS =
  "acp-content-markdown rounded-lg border border-notion-border bg-[#1e1e1e] px-4 py-3 text-[13px] leading-relaxed text-slate-200 shadow-inner [&_.hljs]:bg-transparent [&_a]:text-sky-300 [&_blockquote]:border-l-2 [&_blockquote]:border-white/15 [&_blockquote]:pl-3 [&_blockquote]:text-slate-300 [&_code]:rounded [&_code]:bg-white/10 [&_code]:px-1 [&_code]:text-slate-100 [&_li]:marker:text-slate-500 [&_p]:mb-3 [&_p:last-child]:mb-0 [&_pre]:my-3 [&_pre]:border-0 [&_pre]:bg-transparent [&_pre]:p-0 [&_pre]:text-inherit [&_strong]:text-white";
const ACP_PAYLOAD_SEGMENTED_CLASS = "acp-payload-segmented grid gap-1.5";
const ACP_PAYLOAD_LIST_CLASS = "acp-payload-list m-0 grid list-none gap-1.5 pl-0";
const ACP_PAYLOAD_LIST_ITEM_CLASS = "m-0 min-w-0";
const ACP_PAYLOAD_NESTED_CLASS = "acp-payload-nested rounded-md border border-slate-200 bg-white";
const ACP_PAYLOAD_NESTED_SUMMARY_CLASS =
  "cursor-pointer font-mono text-[10.5px] leading-[1.35] text-slate-500";
const ACP_PAYLOAD_NESTED_BODY_CLASS =
  "acp-payload-nested-body mt-1 border-t border-slate-200 px-2 py-2";
const ACP_SEGMENTED_BLOCK_CLASS = "acp-segmented-block grid gap-1.5";
const ACP_SEGMENTED_FOOTER_CLASS =
  "acp-segmented-footer flex flex-wrap items-center justify-between gap-1.5";
const ACP_SEGMENTED_META_CLASS = "acp-segmented-meta text-[11px] text-slate-500";
const FAILED_TOOL_STATUSES = new Set([
  "failed",
  "cancelled",
  "canceled",
  "interrupted",
  "stopped",
]);
const ansiSegmentCache = new Map<string, AnsiSegment[]>();
const ansiSegmentCacheSize = new Map<string, number>();
let ansiCacheBytes = 0;
let ansiCacheHitCount = 0;
let ansiCacheMissCount = 0;
let payloadParseCount = 0;
let payloadParseFailureCount = 0;

type CacheStats = {
  markdownHits: number;
  markdownMisses: number;
  ansiHits: number;
  ansiMisses: number;
  payloadParses: number;
  payloadParseFailures: number;
};

export function resetAcpConversationCaches(): void {
  resetThreadMarkdownCache();
  ansiSegmentCache.clear();
  ansiSegmentCacheSize.clear();
  ansiCacheBytes = 0;
  ansiCacheHitCount = 0;
  ansiCacheMissCount = 0;
  payloadParseCount = 0;
  payloadParseFailureCount = 0;
}

export function getAcpConversationCacheStats(): CacheStats {
  const markdownStats = getThreadMarkdownCacheStats();
  return {
    markdownHits: markdownStats.markdownHits,
    markdownMisses: markdownStats.markdownMisses,
    ansiHits: ansiCacheHitCount,
    ansiMisses: ansiCacheMissCount,
    payloadParses: payloadParseCount,
    payloadParseFailures: payloadParseFailureCount,
  };
}

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
      className="acp-conversation min-h-0 flex-1 overflow-auto px-0 py-1.5"
      data-acp-conversation-scroll="true"
      ref={containerRef}
      onScroll={onScroll}
      style={conversationScrollStyle}
    >
      <div className="acp-conversation-inner flex w-full flex-col gap-2">
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

const ACP_TOOL_ROW_CLASS = "flex w-full px-4 py-1.5 sm:px-8";
const ACP_TOOL_CARD_CLASS =
  "self-start max-w-[min(88%,78ch)] overflow-hidden rounded-[18px] border border-black/[0.06] bg-white/94 shadow-[0_1px_3px_rgba(15,23,42,0.04)]";
const ACP_TOOL_CARD_NESTED_CLASS =
  "max-w-full border-black/[0.05] bg-notion-sidebar/32 shadow-none";
const ACP_TOOL_SUMMARY_CLASS =
  "flex cursor-pointer list-none items-start gap-3 px-4 py-3 [&::-webkit-details-marker]:hidden";
const ACP_TOOL_TITLE_CLASS =
  "min-w-0 flex-1 text-[13px] font-semibold leading-5 text-notion-text";
const ACP_TOOL_GROUP_LIST_CLASS = "flex flex-col gap-2 px-3 pb-3";

type ToolCallBubbleProps = {
  msg: ToolCallConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  grouped?: boolean;
  indexLabel?: string;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

const ToolCallBubble = React.memo(
  function ToolCallBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    grouped = false,
    indexLabel,
    onSubmitRequestUserInput,
  }: ToolCallBubbleProps) {
    const isLive = isToolCallEffectivelyLive(msg.status, runStatus);
    const [open, setOpen] = React.useState(() => !autoCollapse && isLive);
    const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
    const handleAutoCollapse = React.useCallback(() => {
      setOpen((prev) => (prev ? false : prev));
    }, []);
    const wasLiveRef = React.useRef(isLive);
    const wasAutoCollapseRef = React.useRef(autoCollapse);
    const callHint = deriveToolCallHint(msg.title, msg.raw_input, msg.content);
    const requestUserInputQuestions = React.useMemo(
      () => parseRequestUserInputQuestions(msg.id, msg.raw_input),
      [msg.id, msg.raw_input]
    );
    const requestUserInputResponse = React.useMemo(
      () => parseRequestUserInputResponse(msg.raw_output),
      [msg.raw_output]
    );
    const requestUserInputSummary = React.useMemo(
      () =>
        requestUserInputQuestions
          ? formatRequestUserInputSummary(requestUserInputQuestions)
          : "",
      [requestUserInputQuestions]
    );
    const contentText = React.useMemo(
      () => (msg.content ? unescapeLineBreaks(msg.content) : ""),
      [msg.content]
    );
    const contentShouldDefaultOpen = React.useMemo(
      () => shouldAutoExpandToolContent(contentText),
      [contentText]
    );
    const inputPayload = React.useMemo(
      () => normalizeToolPayload(msg.raw_input),
      [msg.raw_input]
    );
    const outputPayloadSource = React.useMemo(
      () => selectToolCallOutputForDisplay(msg.title, msg.raw_output),
      [msg.title, msg.raw_output]
    );
    const outputPayload = React.useMemo(
      () => normalizeToolPayload(outputPayloadSource),
      [outputPayloadSource]
    );
    const inputPreview = React.useMemo(
      () => summarizeToolPayload(inputPayload, TOOL_PAYLOAD_PREVIEW_LIMIT),
      [inputPayload]
    );
    const outputPreview = React.useMemo(
      () => summarizeToolPayload(outputPayload, TOOL_PAYLOAD_PREVIEW_LIMIT),
      [outputPayload]
    );
    const statusLabel = formatToolCallStatus(msg.status);
    const durationLabel = React.useMemo(
      () => formatToolCallDurationLabel(msg.raw_output),
      [msg.raw_output]
    );
    const statusMark = getToolCallStatusMark(msg.status);
    const terminalActivityPreview = React.useMemo(() => {
      const last = msg.terminal_activities?.at(-1);
      return last ? formatTerminalActivityLabel(last) : "";
    }, [msg.terminal_activities]);

    React.useEffect(() => {
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [isLive]);
    React.useEffect(() => {
      if (autoCollapse && !wasAutoCollapseRef.current) {
        setOpen(false);
      }
      wasAutoCollapseRef.current = autoCollapse;
    }, [autoCollapse]);
    useAutoCollapseToolFoldWhenOutOfView({
      detailsRef,
      enabled: !grouped,
      onCollapse: handleAutoCollapse,
    });

    const title = grouped
      ? `${indexLabel ? `${indexLabel} ` : ""}${msg.title || "Tool Call"}`
      : `Tool Call${msg.title ? `: ${msg.title}` : ""}`;
    const effectiveHint = requestUserInputSummary || callHint;
    const hasRequestUserInputCard =
      requestUserInputQuestions != null && requestUserInputQuestions.length > 0;
    const showPendingRequestUserInputCard = isLive && hasRequestUserInputCard;
    const showResolvedRequestUserInputCard = !isLive && hasRequestUserInputCard;

    return (
      <div
        className={grouped ? "acp-bubble tool_call acp-tool-group-entry my-1" : `acp-bubble tool_call tool-call-enter ${ACP_TOOL_ROW_CLASS}`}
      >
        <details
          className={`acp-tool-fold${grouped ? " acp-tool-fold-nested" : ""} ${ACP_TOOL_CARD_CLASS} ${grouped ? ACP_TOOL_CARD_NESTED_CLASS : ""}`}
          ref={detailsRef}
          open={open}
          onToggle={(event) => {
            setOpen(event.currentTarget.open);
          }}
        >
          <summary className={ACP_TOOL_SUMMARY_CLASS}>
            <span className={`${ACP_TOOL_TITLE_CLASS} flex items-start gap-2`}>
              {statusMark && (
                <span
                  className={`acp-tool-status-mark tone-${statusMark.tone} mt-1 inline-flex h-2 w-2 rounded-full ${statusMark.tone === "success" ? "bg-emerald-500" : "bg-rose-500"}`}
                  title={statusMark.label}
                  aria-label={statusMark.label}
                >
                  <span className="acp-tool-status-dot" />
                </span>
              )}
              <span className="min-w-0 flex-1">
                {title}
                {effectiveHint ? (
                  <span className="ml-2 font-normal text-notion-text-muted">· {effectiveHint}</span>
                ) : ""}
              </span>
            </span>
            {msg.status && (
              <span
                className={`${ACP_TOOL_STATUS_CLASS} ${ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS}`}
              >
                {statusLabel}
                {durationLabel ? ` · ${durationLabel}` : ""}
              </span>
            )}
          </summary>
          <div className="px-3 pb-3">
            {showPendingRequestUserInputCard && requestUserInputQuestions ? (
              <RequestUserInputCard
                toolCallId={msg.id}
                questions={requestUserInputQuestions}
                canSubmit={typeof onSubmitRequestUserInput === "function"}
                onSubmitRequestUserInput={onSubmitRequestUserInput}
              />
            ) : null}
            {showResolvedRequestUserInputCard && requestUserInputQuestions ? (
              <RequestUserInputResultCard
                questions={requestUserInputQuestions}
                response={requestUserInputResponse}
                status={msg.status}
              />
            ) : null}
            {!hasRequestUserInputCard && msg.content && (
              <FoldSection
                key="content"
                label="Content"
                preview={formatConversationPreview(contentText, 88)}
                defaultOpen={contentShouldDefaultOpen}
                lazyRender={true}
              >
                <ToolTextContent
                  text={contentText}
                  markdownClassName={ACP_CONTENT_MARKDOWN_CLASS}
                  tone="terminal"
                />
              </FoldSection>
            )}
            {!hasRequestUserInputCard && hasToolPayload(inputPayload) && (
              <FoldSection
                key="input"
                label="Input"
                preview={inputPreview}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={true}
              >
                <ToolPayloadView payload={inputPayload} />
              </FoldSection>
            )}
            {!hasRequestUserInputCard && hasToolPayload(outputPayload) && (
              <FoldSection
                key="output"
                label="Output"
                preview={outputPreview}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={true}
              >
                <ToolPayloadView payload={outputPayload} />
              </FoldSection>
            )}
            {msg.id && (
              <FoldSection
                key="detailed"
                label="Detailed"
                preview={`call_id=${formatConversationPreview(msg.id, 40)}`}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={false}
              >
                <ToolCallDetailsView
                  details={extractToolCallDetails(msg.id, msg.raw_output, msg.title)}
                />
              </FoldSection>
            )}
            {msg.terminal_activities && msg.terminal_activities.length > 0 && (
              <FoldSection
                key="activity"
                label="Activity"
                preview={terminalActivityPreview}
                defaultOpen={false}
                lazyRender={true}
              >
                <div className="space-y-1 text-sm text-notion-text-muted">
                  {msg.terminal_activities.map((activity, index) => (
                    <div key={`${activity.kind}:${activity.command ?? ""}:${index}`}>
                      {formatTerminalActivityLabel(activity)}
                    </div>
                  ))}
                </div>
              </FoldSection>
            )}
            {msg.terminal_output && (
              <FoldSection
                key="terminal"
                label="Terminal"
                preview={formatConversationPreview(unescapeLineBreaks(msg.terminal_output), 88)}
                defaultOpen={false}
                lazyRender={true}
              >
                <TerminalOutputView
                  text={unescapeLineBreaks(msg.terminal_output)}
                  ansi={ansi}
                />
              </FoldSection>
            )}
          </div>
        </details>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus &&
    prev.autoCollapse === next.autoCollapse &&
    prev.grouped === next.grouped &&
    prev.indexLabel === next.indexLabel &&
    prev.onSubmitRequestUserInput === next.onSubmitRequestUserInput
);

function RequestUserInputCard({
  toolCallId,
  questions,
  canSubmit,
  onSubmitRequestUserInput,
}: {
  toolCallId: string;
  questions: RequestUserInputQuestion[];
  canSubmit: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
}) {
  const [drafts, setDrafts] = React.useState<RequestUserInputDrafts>(() =>
    createInitialRequestUserInputDrafts(questions)
  );
  const [submitting, setSubmitting] = React.useState(false);
  const [errorText, setErrorText] = React.useState<string | null>(null);
  const questionsResetKey = createRequestUserInputQuestionsResetKey(questions);
  const resetStateKey = `${toolCallId}::${questionsResetKey}`;
  const lastResetStateKeyRef = React.useRef<string | null>(null);

  React.useEffect(() => {
    if (lastResetStateKeyRef.current === resetStateKey) {
      return;
    }
    lastResetStateKeyRef.current = resetStateKey;
    setDrafts(createInitialRequestUserInputDrafts(questions));
    setSubmitting(false);
    setErrorText(null);
  }, [questions, resetStateKey]);

  const handleOptionChange = React.useCallback(
    (questionId: string, optionLabel: string) => {
      setDrafts((prev) => ({
        ...prev,
        [questionId]: {
          selectedOptionLabel: optionLabel,
          note: prev[questionId]?.note ?? "",
        },
      }));
      setErrorText(null);
    },
    []
  );

  const handleNoteChange = React.useCallback((questionId: string, note: string) => {
    setDrafts((prev) => ({
      ...prev,
      [questionId]: {
        selectedOptionLabel: prev[questionId]?.selectedOptionLabel ?? null,
        note,
      },
    }));
    setErrorText(null);
  }, []);

  const handleSubmit = React.useCallback(async () => {
    if (!onSubmitRequestUserInput) {
      return;
    }
    const submission = buildRequestUserInputSubmissionText(questions, drafts);
    if (!submission.text) {
      setErrorText("Answer required before continuing.");
      return;
    }
    if (submission.missingQuestionIds.length > 0) {
      setErrorText(
        `Answer required for: ${submission.missingQuestionIds.join(", ")}.`
      );
      return;
    }

    try {
      setSubmitting(true);
      setErrorText(null);
      await onSubmitRequestUserInput(submission.text);
    } catch (error) {
      setErrorText(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  }, [drafts, onSubmitRequestUserInput, questions]);

  return (
    <div className="mx-0 mb-3 mt-2 rounded-xl border border-notion-border bg-notion-sidebar/30 p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-sm font-bold text-notion-text">
            Input Required
          </div>
          <div className="text-xs text-notion-text-muted">
            Submit your answer to continue execution.
          </div>
        </div>
        <span className="rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
          Pending
        </span>
      </div>
      <div className="mt-4 space-y-4">
        {questions.map((question, index) => {
          const draft = drafts[question.id] ?? {
            selectedOptionLabel: null,
            note: "",
          };
          const hasOptions = question.options != null && question.options.length > 0;
          const questionHeaderId = `${toolCallId}:${question.id}:header`;
          const questionPromptId = `${toolCallId}:${question.id}:prompt`;
          const questionTextareaId = `${toolCallId}:${question.id}:note`;
          return (
            <div
              key={question.id}
              className="rounded-lg border border-notion-border bg-white p-4 shadow-sm"
              data-request-user-input-question={question.id}
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="rounded-md bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
                  {questions.length > 1 ? `Q${index + 1}` : "Question"}
                </span>
                <span
                  id={questionHeaderId}
                  className="text-sm font-bold text-notion-text"
                >
                  {question.header || question.id}
                </span>
                {question.isSecret ? (
                  <span className="rounded-md bg-rose-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-rose-600">
                    Secret
                  </span>
                ) : null}
              </div>
              <p
                id={questionPromptId}
                className="mt-2 text-[14px] leading-relaxed text-notion-text"
              >
                {question.question}
              </p>
              {hasOptions ? (
                <div className="mt-4 space-y-2">
                  {question.options?.map((option) => {
                    const checked = draft.selectedOptionLabel === option.label;
                    return (
                      <label
                        key={option.label}
                        className={`flex cursor-pointer items-start gap-3 rounded-md border p-3 transition ${
                          checked
                            ? "border-notion-accent bg-notion-accent-bg"
                            : "border-notion-border bg-white hover:bg-notion-hover"
                        }`}
                      >
                        <input
                          type="radio"
                          name={`${toolCallId}:${question.id}`}
                          value={option.label}
                          checked={checked}
                          onChange={() => handleOptionChange(question.id, option.label)}
                          disabled={submitting}
                          data-request-user-input-option={option.label}
                        />
                        <span className="min-w-0">
                          <span className="block text-sm font-bold text-notion-text">
                            {option.label}
                          </span>
                          <span className="mt-0.5 block text-xs leading-relaxed text-notion-text-muted">
                            {option.description}
                          </span>
                        </span>
                      </label>
                    );
                  })}
                  {question.isOther ? (
                    <label
                      className={`flex cursor-pointer items-start gap-3 rounded-md border p-3 transition ${
                        draft.selectedOptionLabel === REQUEST_USER_INPUT_OTHER_OPTION_LABEL
                          ? "border-notion-accent bg-notion-accent-bg"
                          : "border-notion-border bg-white hover:bg-notion-hover"
                      }`}
                    >
                      <input
                        type="radio"
                        name={`${toolCallId}:${question.id}`}
                        value={REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                        checked={
                          draft.selectedOptionLabel === REQUEST_USER_INPUT_OTHER_OPTION_LABEL
                        }
                        onChange={() =>
                          handleOptionChange(
                            question.id,
                            REQUEST_USER_INPUT_OTHER_OPTION_LABEL
                          )
                        }
                        disabled={submitting}
                        data-request-user-input-option={REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                      />
                      <span className="min-w-0">
                        <span className="block text-sm font-bold text-notion-text">
                          {REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                        </span>
                        <span className="mt-0.5 block text-xs leading-relaxed text-notion-text-muted">
                          Provide custom input in the field below.
                        </span>
                      </span>
                    </label>
                  ) : null}
                </div>
              ) : null}
              <textarea
                id={questionTextareaId}
                className="mono mt-4 min-h-24 w-full rounded-md border border-notion-border bg-white px-3 py-2 text-[13px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10"
                name={questionTextareaId}
                aria-labelledby={`${questionHeaderId} ${questionPromptId}`}
                value={draft.note}
                onChange={(event) => handleNoteChange(question.id, event.currentTarget.value)}
                placeholder={
                  hasOptions
                    ? question.isOther
                      ? "Custom answer or details..."
                      : "Optional notes..."
                    : "Type your answer..."
                }
                disabled={submitting}
                data-request-user-input-note={question.id}
              />
              {question.isSecret ? (
                <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-3`}>
                  Secret answers are submitted but not persisted in history.
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
      {errorText ? (
        <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-4`}>
          {errorText}
        </div>
      ) : null}
      <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
        <p className="text-[11px] leading-relaxed text-notion-text-muted italic max-w-sm">
          Response will be sent through the active turn.
        </p>
        <button
          type="button"
          className="inline-flex h-9 items-center justify-center rounded-md bg-notion-accent px-4 text-[13px] font-bold text-white shadow-sm transition hover:bg-notion-accent/90 disabled:opacity-50 active:translate-y-px"
          onClick={() => {
            void handleSubmit();
          }}
          disabled={!canSubmit || submitting}
          data-request-user-input-submit={toolCallId}
        >
          {submitting ? "Submitting..." : "Submit Answer"}
        </button>
      </div>
    </div>
  );
}

function createRequestUserInputQuestionsResetKey(
  questions: RequestUserInputQuestion[]
): string {
  return JSON.stringify(
    questions.map((question) => ({
      id: question.id,
      header: question.header ?? null,
      question: question.question,
      isOther: question.isOther,
      isSecret: question.isSecret,
      options:
        question.options?.map((option) => ({
          label: option.label,
          description: option.description,
        })) ?? null,
    }))
  );
}

function RequestUserInputResultCard({
  questions,
  response,
  status,
}: {
  questions: RequestUserInputQuestion[];
  response: RequestUserInputResponse | null;
  status?: string;
}) {
  const answeredCount = React.useMemo(
    () => countAnsweredRequestUserInputQuestions(questions, response),
    [questions, response]
  );
  const hasSecretQuestions = questions.some((question) => question.isSecret);
  const hideAllAnswers = hasSecretQuestions && response == null;
  const statusLabel = formatToolCallStatus(status);

  return (
    <div className="mx-0 mb-3 mt-2 rounded-xl border border-notion-border bg-notion-sidebar/30 p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-sm font-bold text-notion-text">
            {questions.length === 1 ? "Question answered" : "Questions answered"}
          </div>
          <div className="text-xs text-notion-text-muted">
            {answeredCount}/{questions.length} answers recorded
            {statusLabel ? ` · ${statusLabel}` : ""}
          </div>
        </div>
        <span className="rounded-sm bg-emerald-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-emerald-700">
          Complete
        </span>
      </div>
      <div className="mt-4 space-y-4">
        {questions.map((question, index) => (
          <RequestUserInputResultQuestion
            key={question.id}
            question={question}
            index={index}
            totalQuestions={questions.length}
            answer={response?.answers[question.id]}
            hideAnswer={hideAllAnswers || question.isSecret}
          />
        ))}
      </div>
      {hideAllAnswers ? (
        <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-4`}>
          Agent suppressed the structured answer payload in execution history.
        </div>
      ) : null}
    </div>
  );
}

function RequestUserInputResultQuestion({
  question,
  index,
  totalQuestions,
  answer,
  hideAnswer,
}: {
  question: RequestUserInputQuestion;
  index: number;
  totalQuestions: number;
  answer: RequestUserInputResponse["answers"][string] | undefined;
  hideAnswer: boolean;
}) {
  const parts = splitRequestUserInputAnswer(answer);
  const hasOptions = question.options != null && question.options.length > 0;
  const hasStructuredAnswer =
    parts.options.length > 0 || (parts.note != null && parts.note.length > 0);

  return (
    <div
      className="rounded-lg border border-notion-border bg-white p-4 shadow-sm"
      data-request-user-input-result={question.id}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="rounded-md bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
          {totalQuestions > 1 ? `Q${index + 1}` : "Question"}
        </span>
        <span className="text-sm font-bold text-notion-text">
          {question.header || question.id}
        </span>
        {question.isSecret ? (
          <span className="rounded-md bg-rose-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-rose-600">
            Secret
          </span>
        ) : null}
      </div>
      <p className="mt-2 text-[14px] leading-relaxed text-notion-text">{question.question}</p>
      {hideAnswer ? (
        <div className="mt-3 rounded-md border border-state-warning-border bg-state-warning-bg px-3 py-2 text-[12px] text-state-warning-text italic">
          Answer submitted privately.
        </div>
      ) : hasStructuredAnswer ? (
        <div className="mt-4 space-y-3">
          {hasOptions ? (
            <div className="flex flex-wrap gap-2">
              {parts.options.map((entry) => (
                <span
                  key={entry}
                  className="rounded-md bg-notion-accent-bg px-2 py-0.5 text-[12px] font-bold text-notion-accent border border-notion-accent/10"
                >
                  {entry}
                </span>
              ))}
            </div>
          ) : (
            <div className="space-y-2">
              {parts.options.map((entry) => (
                <div
                  key={entry}
                  className="mono rounded-md border border-notion-border bg-notion-sidebar/20 px-3 py-2 text-[13px] text-notion-text"
                >
                  {entry}
                </div>
              ))}
            </div>
          )}
          {parts.note ? (
            <div className="mono rounded-md border border-notion-border bg-notion-sidebar/20 px-3 py-2 text-[13px] text-notion-text">
              {parts.note}
            </div>
          ) : null}
        </div>
      ) : (
        <div className="mt-3 rounded-md border border-notion-border bg-notion-sidebar/10 px-3 py-2 text-[12px] text-notion-text-muted italic">
          No payload recorded.
        </div>
      )}
    </div>
  );
}

function formatTerminalActivityLabel(activity: AcpTerminalActivity): string {
  const base =
    activity.kind === "waiting"
      ? "Waiting for background terminal"
      : activity.kind === "waited"
        ? "Waited for background terminal"
        : "Interacted with background terminal";
  if (!activity.command?.trim()) {
    return base;
  }
  return `${base} · ${activity.command.trim()}`;
}

type ToolCallGroupBubbleProps = {
  msg: ToolCallGroupConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

const ToolCallGroupBubble = React.memo(
  function ToolCallGroupBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    onSubmitRequestUserInput,
  }: ToolCallGroupBubbleProps) {
    const isLive = React.useMemo(
      () => msg.calls.some((call) => isToolCallEffectivelyLive(call.status, runStatus)),
      [msg.calls, runStatus]
    );
    const [open, setOpen] = React.useState(() => !autoCollapse && isLive);
    const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
    const handleAutoCollapse = React.useCallback(() => {
      setOpen((prev) => (prev ? false : prev));
    }, []);
    const wasLiveRef = React.useRef(isLive);
    const wasAutoCollapseRef = React.useRef(autoCollapse);
    const titlePreview = React.useMemo(() => summarizeToolGroupTitles(msg.calls), [msg.calls]);
    const statusSummary = React.useMemo(
      () => deriveToolGroupStatusSummary(msg.calls, runStatus),
      [msg.calls, runStatus]
    );

    React.useEffect(() => {
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [isLive]);
    React.useEffect(() => {
      if (autoCollapse && !wasAutoCollapseRef.current) {
        setOpen(false);
      }
      wasAutoCollapseRef.current = autoCollapse;
    }, [autoCollapse]);
    useAutoCollapseToolFoldWhenOutOfView({
      detailsRef,
      enabled: true,
      onCollapse: handleAutoCollapse,
    });

    return (
      <div className="acp-row group relative flex w-full flex-col items-start px-4 py-1.5 sm:px-8">
        <div className={`acp-bubble tool_call tool_call_group tool-call-enter ${ACP_TOOL_CARD_CLASS}`}>
          <details
            className="acp-tool-group-fold"
            ref={detailsRef}
            open={open}
            onToggle={(event) => {
              setOpen(event.currentTarget.open);
            }}
          >
            <summary className={ACP_TOOL_SUMMARY_CLASS}>
              <span className={`${ACP_TOOL_TITLE_CLASS} acp-tool-group-title`}>
                Tool Calls ({msg.calls.length})
                {titlePreview ? (
                  <span className="ml-2 font-normal text-notion-text-muted">· {titlePreview}</span>
                ) : ""}
              </span>
              {statusSummary && (
                <span className={resolveToolGroupStatusClassName(statusSummary.tone)}>
                  {statusSummary.label}
                </span>
              )}
            </summary>
            <div className={`acp-tool-group-list ${ACP_TOOL_GROUP_LIST_CLASS}`}>
              {msg.calls.map((call, idx) => (
                <div
                  key={`${call.id}:${call.event_id ?? call.seq ?? idx}`}
                  className="acp-tool-group-item"
                  data-tool-call-id={call.id}
                >
                  <ToolCallBubble
                    msg={call}
                    ansi={ansi}
                    runStatus={runStatus}
                    autoCollapse={autoCollapse}
                    grouped={true}
                    indexLabel={`#${idx + 1}`}
                    onSubmitRequestUserInput={onSubmitRequestUserInput}
                  />
                </div>
              ))}
            </div>
          </details>
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus &&
    prev.autoCollapse === next.autoCollapse &&
    prev.onSubmitRequestUserInput === next.onSubmitRequestUserInput
);

type ExploreGroupBubbleProps = {
  msg: ExploreGroupConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

const ExploreGroupBubble = React.memo(
  function ExploreGroupBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    onSubmitRequestUserInput,
  }: ExploreGroupBubbleProps) {
    const calls = React.useMemo(
      () => flattenExploreGroupToolCalls(msg.items),
      [msg.items]
    );
    const isLive = React.useMemo(
      () => calls.some((call) => isToolCallEffectivelyLive(call.status, runStatus)),
      [calls, runStatus]
    );
    const [open, setOpen] = React.useState(() => !autoCollapse && isLive);
    const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
    const handleAutoCollapse = React.useCallback(() => {
      setOpen((prev) => (prev ? false : prev));
    }, []);
    const wasLiveRef = React.useRef(isLive);
    const wasAutoCollapseRef = React.useRef(autoCollapse);
    const titlePreview = React.useMemo(
      () => summarizeExploreGroupPreview(msg.items),
      [msg.items]
    );
    const statusSummary = React.useMemo(
      () => deriveToolGroupStatusSummary(calls, runStatus),
      [calls, runStatus]
    );

    React.useEffect(() => {
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [isLive]);
    React.useEffect(() => {
      if (autoCollapse && !wasAutoCollapseRef.current) {
        setOpen(false);
      }
      wasAutoCollapseRef.current = autoCollapse;
    }, [autoCollapse]);
    useAutoCollapseToolFoldWhenOutOfView({
      detailsRef,
      enabled: true,
      onCollapse: handleAutoCollapse,
    });

    let thinkingIndex = 0;
    let toolIndex = 0;
    return (
      <div className="acp-row group relative flex w-full flex-col items-start px-4 py-1.5 sm:px-8">
        <div className={`acp-bubble tool_call tool_call_group explore_group tool-call-enter ${ACP_TOOL_CARD_CLASS}`}>
          <details
            className="acp-tool-group-fold acp-explore-group-fold"
            ref={detailsRef}
            open={open}
            onToggle={(event) => {
              setOpen(event.currentTarget.open);
            }}
          >
            <summary className={ACP_TOOL_SUMMARY_CLASS}>
              <span className={`${ACP_TOOL_TITLE_CLASS} acp-tool-group-title`}>
                Explore ({calls.length} tools)
                {titlePreview ? (
                  <span className="ml-2 font-normal text-notion-text-muted">· {titlePreview}</span>
                ) : ""}
              </span>
              {statusSummary && (
                <span className={resolveToolGroupStatusClassName(statusSummary.tone)}>
                  {statusSummary.label}
                </span>
              )}
            </summary>
            <div className={`acp-tool-group-list acp-explore-group-list ${ACP_TOOL_GROUP_LIST_CLASS}`}>
              {msg.items.map((item, idx) => {
                if (item.kind === "agent_thinking") {
                  thinkingIndex += 1;
                  return (
                    <ExploreThinkingEntry
                      key={`thinking:${item.event_id ?? item.seq ?? idx}`}
                      item={item}
                      index={thinkingIndex}
                    />
                  );
                }
                const groupedCalls = item.kind === "tool_call" ? [item] : item.calls;
                return (
                  <div
                    key={`tool:${item.event_id ?? item.seq ?? idx}`}
                    className="acp-tool-group-item space-y-2"
                  >
                    {groupedCalls.map((call) => {
                      toolIndex += 1;
                      return (
                        <div key={`${call.id}:${call.event_id ?? call.seq ?? toolIndex}`} data-tool-call-id={call.id}>
                          <ToolCallBubble
                            msg={call}
                            ansi={ansi}
                            runStatus={runStatus}
                            autoCollapse={autoCollapse}
                            grouped={true}
                            indexLabel={`#${toolIndex}`}
                            onSubmitRequestUserInput={onSubmitRequestUserInput}
                          />
                        </div>
                      );
                    })}
                  </div>
                );
              })}
            </div>
          </details>
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus &&
    prev.autoCollapse === next.autoCollapse &&
    prev.onSubmitRequestUserInput === next.onSubmitRequestUserInput
);

function ExploreThinkingEntry({
  item,
  index,
}: {
  item: Extract<ConversationItem, { kind: "agent_thinking" }>;
  index: number;
}) {
  return (
    <div className="acp-tool-group-item acp-explore-thinking-item">
      <ThinkingBubble
        text={item.text}
        live={item.live}
        summaryPrefix={`Explore #${index}`}
        grouped={true}
      />
    </div>
  );
}

function summarizeExploreGroupPreview(
  items: ExploreGroupConversationItem["items"]
): string {
  const firstThought = items.find((item) => item.kind === "agent_thinking");
  if (!firstThought) return "";
  return formatConversationPreview(unescapeLineBreaks(firstThought.text), 72);
}

export function deriveToolCallOpenState(
  prevOpen: boolean,
  wasLive: boolean,
  isLive: boolean
): boolean {
  if (isLive) return true;
  if (wasLive) return false;
  return prevOpen;
}

export function shouldCollapseToolFoldWhenOutOfView(
  isIntersecting: boolean,
  intersectionRatio?: number | null
): boolean {
  if (isIntersecting) return false;
  if (intersectionRatio != null && Number.isFinite(intersectionRatio)) {
    return intersectionRatio <= TOOL_VISIBILITY_COLLAPSE_THRESHOLD;
  }
  return true;
}

function useAutoCollapseToolFoldWhenOutOfView({
  detailsRef,
  enabled,
  onCollapse,
}: {
  detailsRef: React.RefObject<HTMLDetailsElement>;
  enabled: boolean;
  onCollapse: () => void;
}): void {
  React.useEffect(() => {
    if (!enabled) return;
    if (typeof window === "undefined") return;
    if (typeof window.IntersectionObserver !== "function") return;
    const node = detailsRef.current;
    if (!node) return;
    const root = node.closest(".acp-conversation");
    if (!(root instanceof HTMLElement)) return;
    const observer = new window.IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.target !== node) continue;
          if (
            shouldCollapseToolFoldWhenOutOfView(
              entry.isIntersecting,
              entry.intersectionRatio
            )
          ) {
            onCollapse();
          }
        }
      },
      {
        root,
        threshold: [0, 0.05],
      }
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [detailsRef, enabled, onCollapse]);
}

export function isToolCallEffectivelyLive(
  status?: string,
  runStatus?: string | null
): boolean {
  if (!isToolCallLive(status)) return false;
  if (!isRunTerminalStatus(runStatus)) return true;
  return false;
}

export function isRunTerminalStatus(status?: string | null): boolean {
  if (!status) return false;
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  return (
    normalized === "completed" ||
    normalized === "failed" ||
    normalized === "cancelled" ||
    normalized === "canceled" ||
    normalized === "stopped" ||
    normalized === "interrupted"
  );
}

type FoldSectionProps = {
  label: string;
  preview: string;
  defaultOpen: boolean;
  parentOpen?: boolean;
  lazyRender?: boolean;
  children: React.ReactNode;
};

function FoldSection({
  label,
  preview,
  defaultOpen,
  parentOpen = true,
  lazyRender = false,
  children,
}: FoldSectionProps) {
  const [open, setOpen] = React.useState(defaultOpen);

  React.useEffect(() => {
    setOpen(defaultOpen);
  }, [defaultOpen]);

  React.useEffect(() => {
    if (!parentOpen) {
      setOpen(false);
    }
  }, [parentOpen]);

  const shouldRenderBody = !lazyRender || open || typeof window === "undefined";
  return (
    <div className={ACP_SUBFOLD_CLASS}>
      <details
        open={open}
        onToggle={(event) => {
          setOpen(event.currentTarget.open);
        }}
      >
        <summary className="flex cursor-pointer list-none items-center gap-2 rounded-md bg-notion-sidebar/55 px-2.5 py-1.5 text-[11px] font-bold uppercase tracking-widest text-notion-text-muted">
          <i className={`bi ${open ? "bi-chevron-down" : "bi-chevron-right"}`} aria-hidden="true" />
          <span>{label}</span>
          {preview && !open ? (
            <span className="truncate opacity-60 font-normal normal-case ml-1 max-w-[240px]">
              · {preview}
            </span>
          ) : null}
        </summary>
        {shouldRenderBody ? <div className="mt-2 border-l border-black/[0.06] pl-3">{children}</div> : null}
      </details>
    </div>
  );
}

function deriveToolCallHint(title: string, rawInput: unknown, content?: string): string {
  const normalized = title.trim().toLowerCase();
  if (!rawInput || typeof rawInput !== "object") {
    if (normalized.includes("explore") && content) {
      return formatConversationPreview(unescapeLineBreaks(content), 60);
    }
    return "";
  }
  const value = rawInput as Record<string, unknown>;
  if (normalized.includes("search")) {
    const query =
      typeof value.q === "string"
        ? value.q
        : typeof value.query === "string"
          ? value.query
          : typeof value.keyword === "string"
            ? value.keyword
            : null;
    if (query) return formatConversationPreview(query, 60);
  }
  if (normalized.includes("explore")) {
    const goal =
      typeof value.goal === "string"
        ? value.goal
        : typeof value.target === "string"
          ? value.target
          : typeof value.topic === "string"
            ? value.topic
            : null;
    if (goal) return formatConversationPreview(goal, 60);
  }
  return "";
}

type NormalizedToolPayload =
  | { kind: "empty" }
  | { kind: "text"; text: string }
  | { kind: "json_text"; text: string; parsed?: unknown }
  | { kind: "json"; value: unknown };

function normalizeToolPayload(value: unknown): NormalizedToolPayload {
  if (value == null) return { kind: "empty" };
  if (typeof value === "string") {
    const text = unescapeLineBreaks(value).trim();
    if (!text) return { kind: "empty" };
    if (isJsonLikeText(text)) {
      return {
        kind: "json_text",
        text,
        parsed: parseJsonLikeString(text, true),
      };
    }
    return { kind: "text", text };
  }
  return { kind: "json", value };
}

function isJsonLikeText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

function parseJsonLikeString(value: string, trackStats: boolean = false): unknown | undefined {
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return undefined;
  if (trackStats) payloadParseCount += 1;
  try {
    return JSON.parse(trimmed);
  } catch {
    if (trackStats) payloadParseFailureCount += 1;
    return undefined;
  }
}

function hasToolPayload(payload: NormalizedToolPayload): boolean {
  return payload.kind !== "empty";
}

function summarizeToolPayload(payload: NormalizedToolPayload, limit: number): string {
  if (payload.kind === "empty") return "";
  if (payload.kind === "text") return formatConversationPreview(payload.text, limit);
  if (payload.kind === "json_text") {
    if (payload.parsed !== undefined) {
      return formatConversationPreview(summarizePayloadValue(payload.parsed), limit);
    }
    return formatConversationPreview(payload.text.replace(/\s+/g, " "), limit);
  }
  return formatConversationPreview(summarizePayloadValue(payload.value), limit);
}

function summarizePayloadValue(value: unknown): string {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return summarizePayloadValue(normalizedValue);
  }
  if (value == null) return "null";
  if (typeof value === "string") return formatConversationPreview(unescapeLineBreaks(value), 120);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return "Array(0)";
    const sample = value
      .slice(0, 3)
      .map((item) => summarizeScalarValue(item))
      .filter((item) => item.length > 0)
      .join(", ");
    return sample ? `Array(${value.length}) · ${sample}` : `Array(${value.length})`;
  }
  if (isPlainObject(value)) {
    const entries = filterPayloadEntries(value);
    if (entries.length === 0) return "Object(0)";
    const preferredKeys = [
      "q",
      "query",
      "cmd",
      "command",
      "path",
      "url",
      "goal",
      "target",
      "tool",
      "fn",
    ];
    const picked = preferredKeys
      .filter((key) => entries.some(([entryKey]) => entryKey === key))
      .slice(0, 3)
      .map((key) => {
        const found = entries.find(([entryKey]) => entryKey === key);
        const resolved = found ? found[1] : undefined;
        return `${key}=${summarizeScalarValue(resolved)}`;
      })
      .filter((pair) => !pair.endsWith("="));
    if (picked.length > 0) return picked.join(" · ");
    const generic = entries
      .slice(0, 3)
      .map(([key, item]) => `${key}=${summarizeScalarValue(item)}`)
      .filter((pair) => !pair.endsWith("="));
    if (generic.length > 0) return generic.join(" · ");
    return `Object(${entries.length})`;
  }
  return String(value);
}

function summarizeScalarValue(value: unknown): string {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return summarizeScalarValue(normalizedValue);
  }
  if (value == null) return "null";
  if (typeof value === "string") return formatConversationPreview(unescapeLineBreaks(value), 48);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return "Array(0)";
    if (value.length <= 3) {
      const inline = value
        .map((item) => summarizePayloadValue(item))
        .filter((item) => item.length > 0)
        .join(", ");
      if (inline) return inline;
    }
    return `Array(${value.length})`;
  }
  if (isPlainObject(value)) return `Object(${Object.keys(value).length})`;
  return "";
}

function summarizeToolGroupTitles(calls: ToolCallConversationItem[]): string {
  if (calls.length === 0) return "";
  const previews = calls
    .slice(0, 2)
    .map((call) => call.title.trim())
    .filter((title) => title.length > 0);
  if (previews.length === 0) return "";
  if (calls.length <= 2) return previews.join(" · ");
  return `${previews.join(" · ")} +${calls.length - 2} more`;
}

function deriveToolGroupStatusSummary(
  calls: ToolCallConversationItem[],
  runStatus?: string | null
): { label: string; tone: ToolGroupStatusTone } | null {
  if (calls.length === 0) return null;
  let liveCount = 0;
  let failedCount = 0;
  for (const call of calls) {
    if (isToolCallEffectivelyLive(call.status, runStatus)) {
      liveCount += 1;
      continue;
    }
    const normalized = normalizeToolCallStatus(call.status);
    if (FAILED_TOOL_STATUSES.has(normalized)) {
      failedCount += 1;
    }
  }
  if (liveCount > 0) return { label: `${liveCount} running`, tone: "running" };
  if (failedCount > 0) return { label: `${failedCount} failed`, tone: "failure" };
  return { label: `${calls.length} completed`, tone: "success" };
}

function normalizeToolCallStatus(status?: string): string {
  if (!status) return "";
  return status.trim().toLowerCase().replace(/[\s-]+/g, "_");
}

function formatToolCallStatus(status?: string): string {
  if (!status) return "";
  const normalized = normalizeToolCallStatus(status);
  switch (normalized) {
    case "in_progress":
      return "In Progress";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "pending":
      return "Pending";
    case "running":
      return "Running";
    case "cancelled":
    case "canceled":
      return "Cancelled";
    case "interrupted":
      return "Interrupted";
    case "stopped":
      return "Stopped";
    default:
      return status;
  }
}

function getToolCallStatusMark(
  status?: string
): { tone: "success" | "failure"; label: string } | null {
  if (!status) return null;
  const normalized = normalizeToolCallStatus(status);
  if (normalized === "completed") {
    return { tone: "success", label: "Completed" };
  }
  if (FAILED_TOOL_STATUSES.has(normalized)) {
    return { tone: "failure", label: "Failed" };
  }
  return null;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value == null) return false;
  if (Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function normalizePayloadKey(key: string): string {
  return key.toLowerCase().replace(/[_-]+/g, "");
}

function isHiddenPayloadKey(key: string): boolean {
  return TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function isPayloadValueEffectivelyEmpty(value: unknown): boolean {
  if (value == null) {
    return true;
  }
  if (typeof value === "string") {
    return unescapeLineBreaks(value).trim().length === 0;
  }
  if (Array.isArray(value)) return value.length === 0;
  if (isPlainObject(value)) return Object.keys(value).length === 0;
  return false;
}

function isEmptyStdStreamPayloadField(key: string, value: unknown): boolean {
  const normalized = normalizePayloadKey(key);
  if (!TOOL_PAYLOAD_HIDE_EMPTY_STREAM_KEY_NORMALIZED.has(normalized)) return false;
  return isPayloadValueEffectivelyEmpty(value);
}

function findPreferredOutputPayloadKey(
  entries: Array<[string, unknown]>
): string | null {
  for (const normalizedKey of TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED) {
    const matched = entries.find(
      ([key, value]) =>
        normalizePayloadKey(key) === normalizedKey &&
        !isPayloadValueEffectivelyEmpty(value)
    );
    if (matched) return matched[0];
  }
  return null;
}

function filterPayloadEntries(
  value: Record<string, unknown>
): Array<[string, unknown]> {
  const entries = Object.entries(value);
  const preferredOutputKey = findPreferredOutputPayloadKey(entries);
  return entries.filter(
    ([key, item]) => {
      const normalized = normalizePayloadKey(key);
      if (
        TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED.has(normalized) &&
        key !== preferredOutputKey
      ) {
        return false;
      }
      return (
      !isHiddenPayloadKey(key) &&
      !isEmptyStdStreamPayloadField(key, item)
      );
    }
  );
}

function ToolPayloadView({ payload }: { payload: NormalizedToolPayload }) {
  if (payload.kind === "empty") return null;
  if (payload.kind === "text") {
    return <ToolTextContent text={payload.text} markdownClassName={ACP_PAYLOAD_MARKDOWN_CLASS} />;
  }
  if (payload.kind === "json_text") {
    if (payload.parsed === undefined) {
      return <ToolTextContent text={payload.text} markdownClassName={ACP_PAYLOAD_MARKDOWN_CLASS} />;
    }
    return (
      <div className={ACP_PAYLOAD_CARD_CLASS}>
        {renderPayloadValue(payload.parsed, 0)}
      </div>
    );
  }
  return (
    <div className={ACP_PAYLOAD_CARD_CLASS}>
      {renderPayloadValue(payload.value, 0)}
    </div>
  );
}

function ToolCallDetailsView({ details }: { details: ToolCallDetailItem[] }) {
  return (
    <div className={ACP_PAYLOAD_CARD_CLASS}>
      <dl className={ACP_PAYLOAD_GRID_CLASS}>
        {details.map((detail) => (
          <div
            className={ACP_PAYLOAD_ROW_CLASS}
            key={detail.key}
          >
            <dt>{detail.key}</dt>
            <dd className="text-sm text-notion-text font-medium opacity-90">
              <code>{detail.value}</code>
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function renderPayloadValue(value: unknown, depth: number): React.ReactNode {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return renderPayloadValue(normalizedValue, depth);
  }
  if (value == null) {
    return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>null</span>;
  }
  if (typeof value === "string") {
    return <ToolTextContent text={unescapeLineBreaks(value)} markdownClassName={ACP_PAYLOAD_MARKDOWN_CLASS} />;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{String(value)}</span>;
  }
  if (Array.isArray(value)) {
    return <PayloadArrayView value={value} depth={depth} />;
  }
  if (isPlainObject(value)) {
    return <PayloadObjectView value={value} depth={depth} />;
  }
  return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{String(value)}</span>;
}

function PayloadArrayView({ value, depth }: { value: unknown[]; depth: number }) {
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    value.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  if (value.length === 0) return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>[]</span>;
  const allScalar = value.every((item) => !Array.isArray(item) && !isPlainObject(item));
  const visibleItems = value.slice(0, visibleCount);

  if (allScalar) {
    return (
      <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
        <span className={ACP_PAYLOAD_SCALAR_CLASS}>
          {visibleItems.map((item) => summarizeScalarValue(item)).join(", ")}
          {hasMore ? ` … (+${remaining} more)` : ""}
        </span>
        {hasMore && (
          <SegmentedMoreFooter
            remaining={remaining}
            unitLabel="items"
            onShowMore={showMore}
          />
        )}
      </div>
    );
  }

  return (
    <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
      <ul className={ACP_PAYLOAD_LIST_CLASS}>
        {visibleItems.map((item, index) => (
          <li className={ACP_PAYLOAD_LIST_ITEM_CLASS} key={index}>
            {renderNestedPayloadValue(item, depth + 1)}
          </li>
        ))}
      </ul>
      {hasMore && (
        <SegmentedMoreFooter
          remaining={remaining}
          unitLabel="items"
          onShowMore={showMore}
        />
      )}
    </div>
  );
}

function PayloadObjectView({
  value,
  depth,
}: {
  value: Record<string, unknown>;
  depth: number;
}) {
  const entries = filterPayloadEntries(value);
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    entries.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  if (entries.length === 0) return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>{"{}"}</span>;
  const visibleEntries = entries.slice(0, visibleCount);
  return (
    <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
      <dl className={ACP_PAYLOAD_GRID_CLASS}>
        {visibleEntries.map(([key, item]) => (
          <div className={ACP_PAYLOAD_ROW_CLASS} key={key}>
            <dt>{key}</dt>
            <dd className="text-sm text-notion-text font-medium opacity-90">
              {renderPayloadFieldValue(key, item, depth + 1)}
            </dd>
          </div>
        ))}
      </dl>
      {hasMore && (
        <SegmentedMoreFooter
          remaining={remaining}
          unitLabel="fields"
          onShowMore={showMore}
        />
      )}
    </div>
  );
}

function renderPayloadFieldValue(
  key: string,
  value: unknown,
  depth: number
): React.ReactNode {
  if (
    typeof value === "string" &&
    shouldPreferPlainTextForPayloadKey(key)
  ) {
    const text = unescapeLineBreaks(value);
    return <ToolPlainTextView text={text} asciiLike={shouldPreserveAsciiText(text)} />;
  }
  if (isPrimaryOutputPayloadField(key) && typeof value === "string") {
    const text = unescapeLineBreaks(value);
    return <ToolPlainTextView text={text} asciiLike={shouldPreserveAsciiText(text)} />;
  }
  return renderNestedPayloadValue(value, depth);
}

function isPrimaryOutputPayloadField(key: string): boolean {
  return TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function renderNestedPayloadValue(value: unknown, depth: number): React.ReactNode {
  const isStructured = Array.isArray(value) || isPlainObject(value);
  if (isStructured && depth > TOOL_PAYLOAD_MAX_NESTED_DEPTH) {
    return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{summarizePayloadValue(value)}</span>;
  }
  if (isStructured && shouldInlineStructuredPayload(value, depth)) {
    return (
      <div className="acp-payload-inline">
        {renderPayloadValue(value, depth)}
      </div>
    );
  }
  if (isStructured) {
    return (
      <details className={ACP_PAYLOAD_NESTED_CLASS}>
        <summary className={`${ACP_PAYLOAD_NESTED_SUMMARY_CLASS} px-2 py-1.5 list-none flex items-center gap-2 cursor-pointer`}>
          <i className="bi bi-chevron-right text-[10px]" />
          <span>{summarizePayloadValue(value)}</span>
        </summary>
        <div className={ACP_PAYLOAD_NESTED_BODY_CLASS}>
          {renderPayloadValue(value, depth)}
        </div>
      </details>
    );
  }
  return renderPayloadValue(value, depth);
}

function shouldInlineStructuredPayload(value: unknown, depth: number): boolean {
  if (depth > 2) return false;
  if (Array.isArray(value)) {
    return value.length > 0 && value.length <= 10;
  }
  if (isPlainObject(value)) {
    const size = Object.keys(value).length;
    return size > 0 && size <= 8;
  }
  return false;
}

function ToolTextContent({
  text,
  markdownClassName,
  preferPlainText = false,
  tone = "default",
}: {
  text: string;
  markdownClassName?: string;
  preferPlainText?: boolean;
  tone?: "default" | "terminal";
}) {
  if (shouldRenderDiffText(text)) {
    return <ToolDiffView text={text} />;
  }
  if (preferPlainText) {
    return <ToolPlainTextView text={text} asciiLike={shouldPreserveAsciiText(text)} />;
  }
  const markdownText = shouldRenderMarkdownText(text);
  const tooLargeForMarkdown =
    countLines(text) > TOOL_TEXT_MARKDOWN_FALLBACK_LINES ||
    text.length > TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH;

  if (markdownText && !tooLargeForMarkdown) {
    return (
      <ThreadRichText text={text} className={markdownClassName} />
    );
  }
  if (markdownText && tooLargeForMarkdown) {
    return (
      <div className={ACP_SEGMENTED_BLOCK_CLASS}>
        <div className={ACP_SEGMENTED_NOTE_WARNING_CLASS}>
          Large markdown payload is rendered as plain text for performance.
        </div>
        <ToolPlainTextView text={text} asciiLike={false} tone={tone} />
      </div>
    );
  }
  return (
    <ToolPlainTextView
      text={text}
      asciiLike={shouldPreserveAsciiText(text)}
      tone={tone}
    />
  );
}

function shouldAutoExpandToolContent(text: string): boolean {
  if (!text) return false;
  if (!shouldRenderMarkdownText(text)) return false;
  if (shouldRenderDiffText(text)) return false;
  if (countLines(text) > TOOL_TEXT_MARKDOWN_FALLBACK_LINES) return false;
  if (text.length > TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH) return false;
  return true;
}

function ToolPlainTextView({
  text,
  asciiLike,
  tone = "default",
}: {
  text: string;
  asciiLike: boolean;
  tone?: "default" | "terminal";
}) {
  const lines = React.useMemo(() => text.split("\n"), [text]);
  const { startIndex, endIndex, hasMore, remaining, showMore } = useProgressiveTailWindow(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleText = React.useMemo(
    () => lines.slice(startIndex, endIndex).join("\n"),
    [lines, startIndex, endIndex]
  );
  const className = asciiLike
    ? tone === "terminal"
      ? ACP_CONTENT_TEXT_ASCII_CLASS
      : ACP_PAYLOAD_TEXT_ASCII_CLASS
    : tone === "terminal"
      ? ACP_CONTENT_TEXT_BASE_CLASS
      : ACP_PAYLOAD_TEXT_BASE_CLASS;
  return (
    <div className={ACP_SEGMENTED_BLOCK_CLASS}>
      <pre className={className}>{visibleText}</pre>
      {hasMore && (
        <SegmentedMoreFooter
          remaining={remaining}
          unitLabel="lines"
          onShowMore={showMore}
        />
      )}
    </div>
  );
}

function TerminalOutputView({
  text,
  ansi,
}: {
  text: string;
  ansi: (input: string) => string;
}) {
  const lines = React.useMemo(() => text.split("\n"), [text]);
  const { startIndex, endIndex, hasMore, remaining, showMore } = useProgressiveTailWindow(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleText = React.useMemo(
    () => lines.slice(startIndex, endIndex).join("\n"),
    [lines, startIndex, endIndex]
  );
  const rendered = React.useMemo(
    () => renderAnsiTerminalOutput(ansi(visibleText)),
    [ansi, visibleText]
  );
  return (
    <div className={ACP_SEGMENTED_BLOCK_CLASS}>
      <pre className={ACP_TERMINAL_PRE_CLASS}>{rendered}</pre>
      {hasMore && (
        <SegmentedMoreFooter
          remaining={remaining}
          unitLabel="lines"
          onShowMore={showMore}
        />
      )}
    </div>
  );
}

function countLines(text: string): number {
  if (!text) return 0;
  let count = 1;
  for (let i = 0; i < text.length; i += 1) {
    if (text.charCodeAt(i) === 10) count += 1;
  }
  return count;
}

function useProgressiveVisibleCount(
  total: number,
  initial: number,
  step: number
): {
  visibleCount: number;
  hasMore: boolean;
  remaining: number;
  showMore: () => void;
} {
  const safeInitial = Math.max(1, initial);
  const safeStep = Math.max(1, step);
  const [visibleCount, setVisibleCount] = React.useState(() =>
    Math.min(total, safeInitial)
  );

  React.useEffect(() => {
    setVisibleCount(Math.min(total, safeInitial));
  }, [total, safeInitial]);

  const showMore = React.useCallback(() => {
    setVisibleCount((prev) => Math.min(total, prev + safeStep));
  }, [safeStep, total]);

  const hasMore = visibleCount < total;
  return {
    visibleCount,
    hasMore,
    remaining: hasMore ? total - visibleCount : 0,
    showMore,
  };
}

function useProgressiveTailWindow(
  total: number,
  initial: number,
  step: number
): {
  startIndex: number;
  endIndex: number;
  hasMore: boolean;
  remaining: number;
  showMore: () => void;
} {
  const safeInitial = Math.max(1, initial);
  const safeStep = Math.max(1, step);
  const baseline = Math.min(total, safeInitial);
  const [visibleCount, setVisibleCount] = React.useState(() => baseline);

  React.useEffect(() => {
    setVisibleCount((prev) => {
      const clampedPrev = Math.min(total, Math.max(prev, 0));
      if (clampedPrev === 0) return baseline;
      return Math.max(baseline, clampedPrev);
    });
  }, [baseline, total]);

  const showMore = React.useCallback(() => {
    setVisibleCount((prev) => Math.min(total, prev + safeStep));
  }, [safeStep, total]);

  const hasMore = visibleCount < total;
  const remaining = hasMore ? total - visibleCount : 0;
  const startIndex = Math.max(0, total - visibleCount);
  return {
    startIndex,
    endIndex: total,
    hasMore,
    remaining,
    showMore,
  };
}

function SegmentedMoreFooter({
  remaining,
  unitLabel,
  onShowMore,
}: {
  remaining: number;
  unitLabel: string;
  onShowMore: () => void;
}) {
  return (
    <div className={ACP_SEGMENTED_FOOTER_CLASS}>
      <span className={ACP_SEGMENTED_META_CLASS}>
        {remaining} more {unitLabel}
      </span>
      <button
        type="button"
        className={ACP_SEGMENTED_BUTTON_CLASS}
        onClick={onShowMore}
        aria-label={`Show ${remaining} more ${unitLabel}`}
      >
        Show more
      </button>
    </div>
  );
}

function shouldRenderMarkdownText(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed) return false;
  if (trimmed.includes("```")) return true;
  if (/`[^`\n]+`/.test(trimmed)) return true;
  if (/^\s{0,3}#{1,6}\s+\S+/m.test(trimmed)) return true;
  if (/^\s{0,3}(?:[-*+]\s+|\d+\.\s+|\d+\)\s+)/m.test(trimmed)) return true;
  if (/^\s{0,3}>\s+[A-Za-z0-9]/m.test(trimmed)) return true;
  if (/^\s{0,3}[-*+]\s+\[[ xX]\]\s+/m.test(trimmed)) return true;
  if (/\[[^\]]+\]\([^)]+\)/.test(trimmed)) return true;
  if (/(^|[\s(])(?:\*\*[^*\n]+\*\*|__[^_\n]+__|\*[^*\n]+\*|_[^_\n]+_)(?=$|[\s).,!?])/m.test(trimmed)) {
    return true;
  }
  if (/^\|.+\|\s*$/m.test(trimmed) && /^\|?[-: ]+\|[-|: ]*$/m.test(trimmed)) {
    return true;
  }
  return false;
}

function shouldPreferPlainTextForPayloadKey(key: string): boolean {
  return TOOL_PAYLOAD_PLAIN_TEXT_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function shouldRenderDiffText(text: string): boolean {
  const normalized = text.trim();
  if (!normalized || !normalized.includes("\n")) return false;
  if (/^diff --git\s+/m.test(normalized)) return true;
  if (/^@@\s+-\d+(?:,\d+)?\s+\+\d+(?:,\d+)?\s+@@/m.test(normalized)) return true;
  if (/^---\s+/m.test(normalized) && /^\+\+\+\s+/m.test(normalized)) return true;
  const lines = normalized.split("\n");
  let add = 0;
  let remove = 0;
  for (const line of lines) {
    if (line.startsWith("+++")) continue;
    if (line.startsWith("---")) continue;
    if (line.startsWith("+")) add += 1;
    if (line.startsWith("-")) remove += 1;
  }
  return add > 0 && remove > 0;
}

function shouldPreserveAsciiText(text: string): boolean {
  const lines = text.split("\n").filter((line) => line.trim().length > 0);
  if (lines.length < 2) return false;
  let symbolicLines = 0;
  for (const line of lines) {
    const compact = line.replace(/\s+/g, "");
    if (compact.length < 3) continue;
    const symbolCount = compact.replace(/[a-zA-Z0-9]/g, "").length;
    if (symbolCount < 2) continue;
    if (symbolCount / compact.length >= 0.35) symbolicLines += 1;
  }
  return symbolicLines >= Math.min(2, lines.length);
}

function normalizeNumericKeyedObject(value: unknown): unknown {
  if (!isPlainObject(value)) return value;
  const keys = Object.keys(value);
  if (keys.length === 0) return value;
  if (!keys.every((key) => /^\d+$/.test(key))) return value;
  const sorted = keys.map((key) => Number(key)).sort((a, b) => a - b);
  const start = sorted[0];
  if (start !== 0 && start !== 1) return value;
  const end = sorted[sorted.length - 1];
  if (end - start + 1 !== sorted.length) return value;
  const normalized: unknown[] = [];
  for (let index = start; index <= end; index += 1) {
    normalized.push(value[String(index)]);
  }
  return normalized;
}

type DiffLineKind = "meta" | "hunk" | "add" | "remove" | "context";

function classifyDiffLine(line: string): DiffLineKind {
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("+") && !line.startsWith("+++")) return "add";
  if (line.startsWith("-") && !line.startsWith("---")) return "remove";
  if (
    line.startsWith("diff --git") ||
    line.startsWith("index ") ||
    line.startsWith("--- ") ||
    line.startsWith("+++ ")
  ) {
    return "meta";
  }
  return "context";
}

function resolveDiffLineToneClassName(kind: DiffLineKind): string {
  switch (kind) {
    case "meta":
      return "text-sky-200 bg-sky-400/15";
    case "hunk":
      return "text-violet-200 bg-violet-400/15";
    case "add":
      return "text-emerald-200 bg-emerald-400/15";
    case "remove":
      return "text-rose-200 bg-rose-400/15";
    case "context":
    default:
      return "text-slate-200";
  }
}

function ToolDiffView({ text }: { text: string }) {
  const lines = React.useMemo(() => text.split("\n"), [text]);
  const { startIndex, endIndex, hasMore, remaining, showMore } = useProgressiveTailWindow(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleLines = lines.slice(startIndex, endIndex);
  return (
    <div className={ACP_SEGMENTED_BLOCK_CLASS}>
      <pre className={ACP_DIFF_PRE_CLASS}>
        {visibleLines.map((line, index) => {
          const kind = classifyDiffLine(line);
          return (
            <span
              className={`acp-diff-line ${kind} block px-1 ${resolveDiffLineToneClassName(kind)}`}
              key={startIndex + index}
            >
              {line.length > 0 ? line : " "}
            </span>
          );
        })}
      </pre>
      {hasMore && (
        <SegmentedMoreFooter
          remaining={remaining}
          unitLabel="lines"
          onShowMore={showMore}
        />
      )}
    </div>
  );
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

const ANSI_SPAN_TAG_PATTERN = /<\/?span(?: style="([a-zA-Z0-9:#;(),.%\s-]*)")?>/g;
const ANSI_STYLE_VALUE_PATTERN = /^[a-zA-Z0-9#(),.%\s-]+$/;
const ANSI_ALLOWED_STYLE_PROPERTIES: Record<string, keyof React.CSSProperties> = {
  color: "color",
  "background-color": "backgroundColor",
  "font-weight": "fontWeight",
  "font-style": "fontStyle",
  "text-decoration": "textDecoration",
  "text-decoration-line": "textDecorationLine",
  "text-decoration-style": "textDecorationStyle",
};

type AnsiSegment = {
  text: string;
  style?: React.CSSProperties;
};

function renderAnsiTerminalOutput(input: string): React.ReactNode[] {
  return parseAnsiSegmentsCached(input).map((segment, index) => {
    if (segment.style) {
      return (
        <span key={index} style={segment.style}>
          {segment.text}
        </span>
      );
    }
    return <React.Fragment key={index}>{segment.text}</React.Fragment>;
  });
}

export function parseAnsiSegmentsCached(input: string): AnsiSegment[] {
  if (input.length > ANSI_SEGMENT_CACHE_MAX_ENTRY_CHARS) {
    ansiCacheMissCount += 1;
    return parseAnsiSegments(input);
  }
  const cached = ansiSegmentCache.get(input);
  if (cached != null) {
    ansiCacheHitCount += 1;
    return cached;
  }
  ansiCacheMissCount += 1;
  const parsed = parseAnsiSegments(input);
  const estimatedBytes = estimateAnsiSegmentsBytes(input, parsed);
  return cacheWithLruBudget(
    ansiSegmentCache,
    ansiSegmentCacheSize,
    () => ansiCacheBytes,
    (next) => {
      ansiCacheBytes = next;
    },
    input,
    parsed,
    estimatedBytes,
    ANSI_SEGMENT_CACHE_LIMIT,
    ANSI_SEGMENT_CACHE_MAX_BYTES
  );
}

function parseAnsiSegments(input: string): AnsiSegment[] {
  const segments: AnsiSegment[] = [];
  const styleStack: React.CSSProperties[] = [];
  let cursor = 0;
  ANSI_SPAN_TAG_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = ANSI_SPAN_TAG_PATTERN.exec(input)) != null) {
    if (match.index > cursor) {
      pushAnsiSegment(segments, input.slice(cursor, match.index), styleStack);
    }
    if (match[0].startsWith("</")) {
      if (styleStack.length > 0) {
        styleStack.pop();
      } else {
        pushAnsiSegment(segments, match[0], styleStack);
      }
    } else {
      styleStack.push(parseAnsiStyle(match[1] ?? ""));
    }
    cursor = ANSI_SPAN_TAG_PATTERN.lastIndex;
  }
  if (cursor < input.length) {
    pushAnsiSegment(segments, input.slice(cursor), styleStack);
  }
  return segments;
}

function cacheWithLruBudget<K, V>(
  cache: Map<K, V>,
  sizes: Map<K, number>,
  currentBytes: () => number,
  setBytes: (next: number) => void,
  key: K,
  value: V,
  size: number,
  entryLimit: number,
  byteLimit: number
): V {
  if (cache.has(key)) {
    const previousSize = sizes.get(key) ?? 0;
    setBytes(Math.max(0, currentBytes() - previousSize));
    sizes.delete(key);
    cache.delete(key);
  }
  cache.set(key, value);
  sizes.set(key, size);
  setBytes(currentBytes() + size);
  while (cache.size > entryLimit || currentBytes() > byteLimit) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey !== undefined) {
      const oldestSize = sizes.get(oldestKey) ?? 0;
      setBytes(Math.max(0, currentBytes() - oldestSize));
      sizes.delete(oldestKey);
      cache.delete(oldestKey);
      continue;
    }
    break;
  }
  return value;
}

function estimateStringBytes(text: string): number {
  return text.length * 2;
}

function estimateAnsiSegmentsBytes(
  input: string,
  segments: AnsiSegment[]
): number {
  let total = estimateStringBytes(input);
  for (const segment of segments) {
    total += estimateStringBytes(segment.text);
    if (!segment.style) continue;
    for (const [key, value] of Object.entries(segment.style)) {
      total += estimateStringBytes(key);
      if (typeof value === "string") {
        total += estimateStringBytes(value);
      } else if (typeof value === "number") {
        total += 8;
      }
    }
  }
  return total;
}

function pushAnsiSegment(
  segments: AnsiSegment[],
  text: string,
  styleStack: React.CSSProperties[]
): void {
  if (!text) return;
  const style = mergeAnsiStyles(styleStack);
  segments.push(style ? { text, style } : { text });
}

function mergeAnsiStyles(styleStack: React.CSSProperties[]): React.CSSProperties | undefined {
  if (styleStack.length === 0) return undefined;
  const merged: React.CSSProperties = {};
  for (const style of styleStack) {
    Object.assign(merged, style);
  }
  return Object.keys(merged).length > 0 ? merged : undefined;
}

function parseAnsiStyle(rawStyle: string): React.CSSProperties {
  const parsed: React.CSSProperties = {};
  for (const entry of rawStyle.split(";")) {
    const separator = entry.indexOf(":");
    if (separator < 0) continue;
    const rawKey = entry.slice(0, separator).trim().toLowerCase();
    const rawValue = entry.slice(separator + 1).trim();
    const targetKey = ANSI_ALLOWED_STYLE_PROPERTIES[rawKey];
    if (!targetKey) continue;
    if (!rawValue || !ANSI_STYLE_VALUE_PATTERN.test(rawValue)) continue;
    (parsed as Record<string, string>)[targetKey] = rawValue;
  }
  return parsed;
}
