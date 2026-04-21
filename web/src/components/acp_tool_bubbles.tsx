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
  ACP_TOOL_STATUS_CLASS,
  ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS,
} from "../ui/tailwind_classes";
import {
  formatRequestUserInputSummary,
  parseRequestUserInputQuestions,
  parseRequestUserInputResponse,
} from "../request_user_input";
import {
  extractToolCallDetails,
  formatToolCallDurationLabel,
  resolveToolGroupStatusClassName,
  selectToolCallOutputForDisplay,
  type ToolGroupStatusTone,
} from "./acp_tool_call_meta";
import {
  TOOL_PAYLOAD_PREVIEW_LIMIT,
  TerminalOutputView,
  ToolCallDetailsView,
  ToolPayloadView,
  ToolTextContent,
  hasToolPayload,
  normalizeToolPayload,
  shouldAutoExpandToolContent,
  summarizeToolPayload,
} from "./acp_tool_content";
import {
  RequestUserInputCard,
  RequestUserInputResultCard,
} from "./acp_request_user_input_cards";
import { ThinkingBubble } from "./bubbles/thinking_bubble";

const TOOL_VISIBILITY_COLLAPSE_THRESHOLD = 0;
const ACP_SUBFOLD_CLASS = "acp-subfold mt-1.5";
const ACP_TOOL_ROW_CLASS = "flex w-full px-3 py-1 sm:px-4";
const ACP_TOOL_CARD_CLASS =
  "self-start max-w-[min(92%,78ch)] overflow-hidden rounded-[18px] border border-slate-200 bg-white shadow-[0_1px_2px_rgba(15,23,42,0.06)]";
const ACP_TOOL_CARD_NESTED_CLASS =
  "max-w-full border-slate-200/80 bg-slate-50/70 shadow-none";
const ACP_TOOL_SUMMARY_CLASS =
  "flex cursor-pointer list-none items-start gap-3 px-4 py-3 [&::-webkit-details-marker]:hidden";
const ACP_TOOL_TITLE_CLASS =
  "min-w-0 flex-1 text-[13px] font-semibold leading-5 text-slate-900";
const ACP_TOOL_GROUP_LIST_CLASS = "flex flex-col gap-2 px-3 pb-3";
const FAILED_TOOL_STATUSES = new Set([
  "failed",
  "cancelled",
  "canceled",
  "interrupted",
  "stopped",
]);

type ToolCallBubbleProps = {
  msg: ToolCallConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  grouped?: boolean;
  indexLabel?: string;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export const ToolCallBubble = React.memo(
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
      setOpen((prevOpen) =>
        deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive)
      );
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
        className={
          grouped
            ? "acp-bubble tool_call acp-tool-group-entry my-1"
            : `acp-bubble tool_call tool-call-enter ${ACP_TOOL_ROW_CLASS}`
        }
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
                  <span className="ml-2 font-normal text-notion-text-muted">
                    · {effectiveHint}
                  </span>
                ) : (
                  ""
                )}
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
                statusLabel={statusLabel}
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
                <ToolTextContent text={contentText} tone="terminal" />
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
                preview={formatConversationPreview(
                  unescapeLineBreaks(msg.terminal_output),
                  88
                )}
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

type ToolCallGroupBubbleProps = {
  msg: ToolCallGroupConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export const ToolCallGroupBubble = React.memo(
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
    const titlePreview = React.useMemo(
      () => summarizeToolGroupTitles(msg.calls),
      [msg.calls]
    );
    const statusSummary = React.useMemo(
      () => deriveToolGroupStatusSummary(msg.calls, runStatus),
      [msg.calls, runStatus]
    );

    React.useEffect(() => {
      setOpen((prevOpen) =>
        deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive)
      );
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
                  <span className="ml-2 font-normal text-notion-text-muted">
                    · {titlePreview}
                  </span>
                ) : (
                  ""
                )}
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

export const ExploreGroupBubble = React.memo(
  function ExploreGroupBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    onSubmitRequestUserInput,
  }: ExploreGroupBubbleProps) {
    const calls = React.useMemo(() => flattenExploreGroupToolCalls(msg.items), [msg.items]);
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
      setOpen((prevOpen) =>
        deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive)
      );
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
                  <span className="ml-2 font-normal text-notion-text-muted">
                    · {titlePreview}
                  </span>
                ) : (
                  ""
                )}
              </span>
              {statusSummary && (
                <span className={resolveToolGroupStatusClassName(statusSummary.tone)}>
                  {statusSummary.label}
                </span>
              )}
            </summary>
            <div
              className={`acp-tool-group-list acp-explore-group-list ${ACP_TOOL_GROUP_LIST_CLASS}`}
            >
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
                        <div
                          key={`${call.id}:${call.event_id ?? call.seq ?? toolIndex}`}
                          data-tool-call-id={call.id}
                        >
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
  detailsRef: React.RefObject<HTMLDetailsElement | null>;
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
          <i
            className={`bi ${open ? "bi-chevron-down" : "bi-chevron-right"}`}
            aria-hidden="true"
          />
          <span>{label}</span>
          {preview && !open ? (
            <span className="ml-1 max-w-[240px] truncate font-normal normal-case opacity-60">
              · {preview}
            </span>
          ) : null}
        </summary>
        {shouldRenderBody ? (
          <div className="mt-2 border-l border-black/[0.06] pl-3">{children}</div>
        ) : null}
      </details>
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
