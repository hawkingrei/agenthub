import React from "react";
import { ConversationItem, formatConversationPreview, isToolCallLive } from "../conversation";
import { renderMarkdown } from "../markdown";

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
  onScroll: () => void;
  containerRef: React.RefObject<HTMLDivElement>;
  ansi: (input: string) => string;
};

const MARKDOWN_CACHE_LIMIT = 512;
const ANSI_SEGMENT_CACHE_LIMIT = 512;
const TOOL_PAYLOAD_PREVIEW_LIMIT = 88;
const TOOL_PAYLOAD_MAX_NESTED_DEPTH = 2;
const TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED = new Set<string>(["turnid"]);
const TOOL_TEXT_INITIAL_LINES = 120;
const TOOL_TEXT_LINE_CHUNK = 220;
const TOOL_TEXT_MARKDOWN_FALLBACK_LINES = 260;
const TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH = 16000;
const TOOL_PAYLOAD_INITIAL_ITEMS = 24;
const TOOL_PAYLOAD_ITEM_CHUNK = 48;

const markdownHtmlCache = new Map<string, string>();
const ansiSegmentCache = new Map<string, AnsiSegment[]>();
let markdownCacheHitCount = 0;
let markdownCacheMissCount = 0;
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
  markdownHtmlCache.clear();
  ansiSegmentCache.clear();
  markdownCacheHitCount = 0;
  markdownCacheMissCount = 0;
  ansiCacheHitCount = 0;
  ansiCacheMissCount = 0;
  payloadParseCount = 0;
  payloadParseFailureCount = 0;
}

export function getAcpConversationCacheStats(): CacheStats {
  return {
    markdownHits: markdownCacheHitCount,
    markdownMisses: markdownCacheMissCount,
    ansiHits: ansiCacheHitCount,
    ansiMisses: ansiCacheMissCount,
    payloadParses: payloadParseCount,
    payloadParseFailures: payloadParseFailureCount,
  };
}

export function renderMarkdownCached(text: string): string {
  const cached = markdownHtmlCache.get(text);
  if (cached != null) {
    markdownCacheHitCount += 1;
    return cached;
  }
  markdownCacheMissCount += 1;
  return cacheWithLruEviction(
    markdownHtmlCache,
    text,
    renderMarkdown(text),
    MARKDOWN_CACHE_LIMIT
  );
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
  onScroll,
  containerRef,
  ansi,
}: AcpConversationProps) {
  return (
    <div className="acp-conversation" ref={containerRef} onScroll={onScroll}>
      <div className="acp-conversation-inner">
        {virtualTopSpacer > 0 && (
          <div
            className="acp-conversation-spacer virtual-top"
            style={{ height: virtualTopSpacer }}
          />
        )}
        {items.map((msg, idx) => {
          const globalIndex = windowOffset + idx;
          const key = getConversationItemKey(msg, globalIndex);
          return (
            <ConversationBubble
              key={key}
              msg={msg}
              globalIndex={globalIndex}
              shouldAutoCollapse={shouldAutoCollapse}
              collapseCutoff={collapseCutoff}
              isFrozenView={isFrozenView}
              runStatus={runStatus}
              ansi={ansi}
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

type ConversationBubbleProps = {
  msg: ConversationItem;
  globalIndex: number;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  isFrozenView: boolean;
  runStatus?: string | null;
  ansi: (input: string) => string;
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
  }: ConversationBubbleProps) {
    const autoCollapse =
      shouldAutoCollapse && !isFrozenView && globalIndex < collapseCutoff;

    if (msg.kind === "agent_thinking") {
      const thinkingLabel = deriveThinkingLabel(msg.text);
      const preview = autoCollapse ? formatConversationPreview(msg.text, 80) : "";
      const summary = msg.live
        ? `${thinkingLabel} (live)`
        : autoCollapse
          ? `${thinkingLabel}: ${preview}`
          : `${thinkingLabel} (collapsed)`;
      return (
        <div className="acp-bubble agent_thinking">
          <details className="acp-thought-fold" open={msg.live}>
            <summary>{summary}</summary>
            <div className="acp-text">
              <pre>{msg.text}</pre>
            </div>
          </details>
        </div>
      );
    }

    if (msg.kind === "agent_plan") {
      return <PlanBubble msg={msg} autoCollapse={autoCollapse} />;
    }

    if (msg.kind === "tool_call") {
      return <ToolCallBubble msg={msg} ansi={ansi} runStatus={runStatus} />;
    }

    if (msg.kind === "agent_message") {
      return <MarkdownBubble className="agent_message" text={msg.text} />;
    }

    return <MarkdownBubble className="user_message" text={msg.text} />;
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
  if (prev.msg.kind === "tool_call") {
    return prev.runStatus === next.runStatus;
  }
  return true;
}

type MarkdownBubbleProps = {
  className: "agent_message" | "user_message";
  text: string;
};

const MarkdownBubble = React.memo(function MarkdownBubble({
  className,
  text,
}: MarkdownBubbleProps) {
  return (
    <div className={`acp-bubble ${className}`}>
      <div
        className="acp-text"
        dangerouslySetInnerHTML={{
          __html: renderMarkdownCached(text),
        }}
      />
    </div>
  );
});

type ToolCallBubbleProps = {
  msg: Extract<ConversationItem, { kind: "tool_call" }>;
  ansi: (input: string) => string;
  runStatus?: string | null;
};

const ToolCallBubble = React.memo(
  function ToolCallBubble({ msg, ansi, runStatus }: ToolCallBubbleProps) {
    const isLive = isToolCallEffectivelyLive(msg.status, runStatus);
    const [open, setOpen] = React.useState(isLive);
    const wasLiveRef = React.useRef(isLive);
    const callHint = deriveToolCallHint(msg.title, msg.raw_input, msg.content);
    const inputPayload = React.useMemo(
      () => normalizeToolPayload(msg.raw_input),
      [msg.raw_input]
    );
    const outputPayload = React.useMemo(
      () => normalizeToolPayload(msg.raw_output),
      [msg.raw_output]
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

    React.useEffect(() => {
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [isLive]);

    return (
      <div className="acp-bubble tool_call">
        <details
          className="acp-tool-fold"
          open={open}
          onToggle={(event) => {
            setOpen(event.currentTarget.open);
          }}
        >
          <summary>
            <span className="acp-tool-title">
              Tool Call
              {msg.title ? `: ${msg.title}` : ""}
              {callHint ? ` · ${callHint}` : ""}
            </span>
            {msg.status && (
              <span className="acp-tool-status">{statusLabel}</span>
            )}
          </summary>
          {msg.content && (
            <FoldSection
              label="Content"
              preview={formatConversationPreview(unescapeLineBreaks(msg.content), 88)}
              defaultOpen={isLive}
              lazyRender={true}
            >
              <ToolTextContent
                text={unescapeLineBreaks(msg.content)}
                markdownClassName="acp-payload-markdown"
              />
            </FoldSection>
          )}
          {hasToolPayload(inputPayload) && (
            <FoldSection
              label="Input"
              preview={inputPreview}
              defaultOpen={false}
              lazyRender={true}
            >
              <ToolPayloadView payload={inputPayload} />
            </FoldSection>
          )}
          {hasToolPayload(outputPayload) && (
            <FoldSection
              label="Output"
              preview={outputPreview}
              defaultOpen={!isLive}
              lazyRender={true}
            >
              <ToolPayloadView payload={outputPayload} />
            </FoldSection>
          )}
          {msg.terminal_output && (
            <FoldSection
              label="Terminal"
              preview={formatConversationPreview(unescapeLineBreaks(msg.terminal_output), 88)}
              defaultOpen={isLive}
              lazyRender={true}
            >
              <TerminalOutputView
                text={unescapeLineBreaks(msg.terminal_output)}
                ansi={ansi}
              />
            </FoldSection>
          )}
        </details>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus
);

export function deriveToolCallOpenState(
  prevOpen: boolean,
  wasLive: boolean,
  isLive: boolean
): boolean {
  if (isLive) return true;
  if (wasLive) return false;
  return prevOpen;
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

function deriveThinkingLabel(text: string): string {
  const firstLine = text
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line.length > 0)
    ?.toLowerCase();
  if (!firstLine) return "Thinking";
  if (firstLine.startsWith("explore")) return "Explore";
  if (firstLine.startsWith("plan")) return "Plan";
  if (firstLine.startsWith("reflect")) return "Reflection";
  return "Thinking";
}

type PlanBubbleProps = {
  msg: Extract<ConversationItem, { kind: "agent_plan" }>;
  autoCollapse: boolean;
};

const PlanBubble = React.memo(
  function PlanBubble({ msg, autoCollapse }: PlanBubbleProps) {
    const planSummary = summarizePlan(msg.plan_entries);
    const preview = autoCollapse ? formatConversationPreview(msg.text, 88) : "";
    const summary = planSummary.total > 0
      ? `Plan: ${planSummary.completed}/${planSummary.total} done · ${planSummary.active} active`
      : autoCollapse
        ? `Plan: ${preview}`
        : "Plan (collapsed)";
    return (
      <div className="acp-bubble agent_plan">
        <details className="acp-thought-fold acp-plan-fold">
          <summary>{summary}</summary>
          <div className="acp-text">
            {planSummary.total > 0 ? (
              <div className="acp-plan-card">
                <div className="acp-plan-progress">
                  <div className="acp-plan-progress-meta">
                    <span>{planSummary.completed}/{planSummary.total} completed</span>
                    <span>{planSummary.active} active</span>
                    <span>{planSummary.pending} pending</span>
                  </div>
                  <div className="acp-plan-progress-bar">
                    <span style={{ width: `${planSummary.ratio}%` }} />
                  </div>
                </div>
                <ol className="acp-plan-list">
                  {msg.plan_entries?.map((entry, idx) => {
                    const status = normalizePlanEntryStatus(entry.status);
                    return (
                      <li key={`${idx}-${entry.content}`} className={`acp-plan-item ${status}`}>
                        <span className="acp-plan-index">{idx + 1}</span>
                        <span className="acp-plan-content">{entry.content}</span>
                        {entry.priority && (
                          <span className="acp-plan-priority">{entry.priority}</span>
                        )}
                        {entry.status && (
                          <span className="acp-plan-status">{entry.status}</span>
                        )}
                      </li>
                    );
                  })}
                </ol>
              </div>
            ) : (
              <pre>{msg.text}</pre>
            )}
          </div>
        </details>
      </div>
    );
  },
  (prev, next) => prev.msg === next.msg && prev.autoCollapse === next.autoCollapse
);

type FoldSectionProps = {
  label: string;
  preview: string;
  defaultOpen: boolean;
  lazyRender?: boolean;
  children: React.ReactNode;
};

function FoldSection({
  label,
  preview,
  defaultOpen,
  lazyRender = false,
  children,
}: FoldSectionProps) {
  const [open, setOpen] = React.useState(defaultOpen);
  const [activated, setActivated] = React.useState(defaultOpen);

  React.useEffect(() => {
    if (!defaultOpen) return;
    setOpen(true);
    setActivated(true);
  }, [defaultOpen]);

  const shouldRenderBody = !lazyRender || activated;
  return (
    <details
      className="acp-subfold"
      open={open}
      onToggle={(event) => {
        const nextOpen = event.currentTarget.open;
        setOpen(nextOpen);
        if (nextOpen) {
          setActivated(true);
        }
      }}
    >
      <summary>
        <span>{label}</span>
        {preview ? <span className="acp-subfold-preview">{preview}</span> : null}
      </summary>
      {shouldRenderBody ? children : null}
    </details>
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

function summarizePlan(
  entries?: Array<{ status?: string }>
): { total: number; completed: number; active: number; pending: number; ratio: number } {
  const total = entries?.length ?? 0;
  if (total === 0) {
    return { total: 0, completed: 0, active: 0, pending: 0, ratio: 0 };
  }
  let completed = 0;
  let active = 0;
  for (const entry of entries ?? []) {
    const status = normalizePlanEntryStatus(entry.status);
    if (status === "completed") completed += 1;
    else if (status === "active") active += 1;
  }
  const pending = Math.max(0, total - completed - active);
  return {
    total,
    completed,
    active,
    pending,
    ratio: Math.round((completed / total) * 100),
  };
}

function normalizePlanEntryStatus(status?: string): "completed" | "active" | "pending" {
  if (!status) return "pending";
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (normalized === "completed" || normalized === "done" || normalized === "finished") {
    return "completed";
  }
  if (normalized === "in_progress" || normalized === "running" || normalized === "active") {
    return "active";
  }
  return "pending";
}

function unescapeLineBreaks(text: string): string {
  return text
    .replace(/\\r\\n/g, "\n")
    .replace(/\\n/g, "\n")
    .replace(/\\t/g, "\t")
    .replace(/\\r/g, "\n");
}

type NormalizedToolPayload =
  | { kind: "empty" }
  | { kind: "text"; text: string }
  | { kind: "json_text"; text: string }
  | { kind: "json"; value: unknown };

function normalizeToolPayload(value: unknown): NormalizedToolPayload {
  if (value == null) return { kind: "empty" };
  if (typeof value === "string") {
    const text = unescapeLineBreaks(value).trim();
    if (!text) return { kind: "empty" };
    if (isJsonLikeText(text)) return { kind: "json_text", text };
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
    const parsed = parseJsonLikeString(payload.text);
    if (parsed !== undefined) {
      return formatConversationPreview(summarizePayloadValue(parsed), limit);
    }
    return formatConversationPreview(payload.text.replace(/\s+/g, " "), limit);
  }
  return formatConversationPreview(summarizePayloadValue(payload.value), limit);
}

function summarizePayloadValue(value: unknown): string {
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
  if (value == null) return "null";
  if (typeof value === "string") return formatConversationPreview(unescapeLineBreaks(value), 48);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return `Array(${value.length})`;
  if (isPlainObject(value)) return `Object(${Object.keys(value).length})`;
  return "";
}

function formatToolCallStatus(status?: string): string {
  if (!status) return "";
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (normalized === "in_progress") return "In Progress";
  if (normalized === "completed") return "Completed";
  if (normalized === "failed") return "Failed";
  if (normalized === "pending") return "Pending";
  if (normalized === "running") return "Running";
  if (normalized === "cancelled" || normalized === "canceled") return "Cancelled";
  if (normalized === "interrupted") return "Interrupted";
  if (normalized === "stopped") return "Stopped";
  return status;
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

function filterPayloadEntries(
  value: Record<string, unknown>
): Array<[string, unknown]> {
  return Object.entries(value).filter(([key]) => !isHiddenPayloadKey(key));
}

function ToolPayloadView({ payload }: { payload: NormalizedToolPayload }) {
  const parsedJsonText = React.useMemo(() => {
    if (payload.kind !== "json_text") return undefined;
    return parseJsonLikeString(payload.text, true);
  }, [payload.kind, payload.kind === "json_text" ? payload.text : ""]);

  if (payload.kind === "empty") return null;
  if (payload.kind === "text") {
    return <ToolTextContent text={payload.text} markdownClassName="acp-payload-markdown" />;
  }
  if (payload.kind === "json_text") {
    if (parsedJsonText === undefined) {
      return <ToolTextContent text={payload.text} markdownClassName="acp-payload-markdown" />;
    }
    return (
      <div className="acp-payload-card">
        {renderPayloadValue(parsedJsonText, 0)}
      </div>
    );
  }
  return (
    <div className="acp-payload-card">
      {renderPayloadValue(payload.value, 0)}
    </div>
  );
}

function renderPayloadValue(value: unknown, depth: number): React.ReactNode {
  if (value == null) {
    return <span className="acp-payload-scalar muted">null</span>;
  }
  if (typeof value === "string") {
    return <ToolTextContent text={unescapeLineBreaks(value)} markdownClassName="acp-payload-markdown" />;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return <span className="acp-payload-scalar">{String(value)}</span>;
  }
  if (Array.isArray(value)) {
    return <PayloadArrayView value={value} depth={depth} />;
  }
  if (isPlainObject(value)) {
    return <PayloadObjectView value={value} depth={depth} />;
  }
  return <span className="acp-payload-scalar">{String(value)}</span>;
}

function PayloadArrayView({ value, depth }: { value: unknown[]; depth: number }) {
  if (value.length === 0) return <span className="acp-payload-scalar muted">[]</span>;
  const allScalar = value.every((item) => !Array.isArray(item) && !isPlainObject(item));
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    value.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  const visibleItems = value.slice(0, visibleCount);

  if (allScalar) {
    return (
      <div className="acp-payload-segmented">
        <span className="acp-payload-scalar">
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
    <div className="acp-payload-segmented">
      <ol className="acp-payload-list">
        {visibleItems.map((item, index) => (
          <li key={index}>
            {renderNestedPayloadValue(item, depth + 1)}
          </li>
        ))}
      </ol>
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
  if (entries.length === 0) return <span className="acp-payload-scalar muted">{"{}"}</span>;
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    entries.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  const visibleEntries = entries.slice(0, visibleCount);
  return (
    <div className="acp-payload-segmented">
      <dl className="acp-payload-grid">
        {visibleEntries.map(([key, item]) => (
          <div className="acp-payload-row" key={key}>
            <dt>{key}</dt>
            <dd>{renderNestedPayloadValue(item, depth + 1)}</dd>
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

function renderNestedPayloadValue(value: unknown, depth: number): React.ReactNode {
  const isStructured = Array.isArray(value) || isPlainObject(value);
  if (isStructured && depth > TOOL_PAYLOAD_MAX_NESTED_DEPTH) {
    return <span className="acp-payload-scalar">{summarizePayloadValue(value)}</span>;
  }
  if (isStructured) {
    return (
      <details className="acp-payload-nested">
        <summary>{summarizePayloadValue(value)}</summary>
        <div className="acp-payload-nested-body">
          {renderPayloadValue(value, depth)}
        </div>
      </details>
    );
  }
  return renderPayloadValue(value, depth);
}

function ToolTextContent({
  text,
  markdownClassName,
}: {
  text: string;
  markdownClassName?: string;
}) {
  if (shouldRenderDiffText(text)) {
    return <ToolDiffView text={text} />;
  }
  const markdownText = shouldRenderMarkdownText(text);
  const tooLargeForMarkdown =
    countLines(text) > TOOL_TEXT_MARKDOWN_FALLBACK_LINES ||
    text.length > TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH;

  if (markdownText && !tooLargeForMarkdown) {
    return (
      <div
        className={`acp-text ${markdownClassName ?? ""}`.trim()}
        dangerouslySetInnerHTML={{ __html: renderMarkdownCached(text) }}
      />
    );
  }
  if (markdownText && tooLargeForMarkdown) {
    return (
      <div className="acp-segmented-block">
        <div className="acp-segmented-note">
          Large markdown payload is rendered as plain text for performance.
        </div>
        <ToolPlainTextView text={text} asciiLike={false} />
      </div>
    );
  }
  return <ToolPlainTextView text={text} asciiLike={shouldPreserveAsciiText(text)} />;
}

function ToolPlainTextView({ text, asciiLike }: { text: string; asciiLike: boolean }) {
  const lines = React.useMemo(() => text.split("\n"), [text]);
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleText = React.useMemo(
    () => lines.slice(0, visibleCount).join("\n"),
    [lines, visibleCount]
  );
  const className = asciiLike
    ? "acp-content acp-payload-text acp-payload-ascii"
    : "acp-content acp-payload-text";
  return (
    <div className="acp-segmented-block">
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
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleText = React.useMemo(
    () => lines.slice(0, visibleCount).join("\n"),
    [lines, visibleCount]
  );
  const rendered = React.useMemo(
    () => renderAnsiTerminalOutput(ansi(visibleText)),
    [ansi, visibleText]
  );
  return (
    <div className="acp-segmented-block">
      <pre className="acp-content">{rendered}</pre>
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
    <div className="acp-segmented-footer">
      <span className="acp-segmented-meta">
        {remaining} more {unitLabel}
      </span>
      <button type="button" className="acp-segmented-button" onClick={onShowMore}>
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
  return false;
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

function ToolDiffView({ text }: { text: string }) {
  const lines = React.useMemo(() => text.split("\n"), [text]);
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    lines.length,
    TOOL_TEXT_INITIAL_LINES,
    TOOL_TEXT_LINE_CHUNK
  );
  const visibleLines = lines.slice(0, visibleCount);
  return (
    <div className="acp-segmented-block">
      <pre className="acp-content acp-diff-view">
        {visibleLines.map((line, index) => {
          const kind = classifyDiffLine(line);
          return (
            <span className={`acp-diff-line ${kind}`} key={`${index}-${line}`}>
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
  if (msg.event_id != null) return `${msg.kind}:event:${msg.event_id}`;
  if (msg.seq) return `${msg.kind}:seq:${msg.seq}`;
  return `${msg.kind}:idx:${fallback}`;
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
  const cached = ansiSegmentCache.get(input);
  if (cached != null) {
    ansiCacheHitCount += 1;
    return cached;
  }
  ansiCacheMissCount += 1;
  return cacheWithLruEviction(
    ansiSegmentCache,
    input,
    parseAnsiSegments(input),
    ANSI_SEGMENT_CACHE_LIMIT
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

function cacheWithLruEviction<K, V>(
  cache: Map<K, V>,
  key: K,
  value: V,
  limit: number
): V {
  if (cache.has(key)) {
    cache.delete(key);
  }
  cache.set(key, value);
  if (cache.size > limit) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey !== undefined) {
      cache.delete(oldestKey);
    }
  }
  return value;
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
