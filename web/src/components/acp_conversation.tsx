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

const markdownHtmlCache = new Map<string, string>();
const ansiSegmentCache = new Map<string, AnsiSegment[]>();
let markdownCacheHitCount = 0;
let markdownCacheMissCount = 0;
let ansiCacheHitCount = 0;
let ansiCacheMissCount = 0;

type CacheStats = {
  markdownHits: number;
  markdownMisses: number;
  ansiHits: number;
  ansiMisses: number;
};

export function resetAcpConversationCaches(): void {
  markdownHtmlCache.clear();
  ansiSegmentCache.clear();
  markdownCacheHitCount = 0;
  markdownCacheMissCount = 0;
  ansiCacheHitCount = 0;
  ansiCacheMissCount = 0;
}

export function getAcpConversationCacheStats(): CacheStats {
  return {
    markdownHits: markdownCacheHitCount,
    markdownMisses: markdownCacheMissCount,
    ansiHits: ansiCacheHitCount,
    ansiMisses: ansiCacheMissCount,
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
              <span className="acp-tool-status">{msg.status}</span>
            )}
          </summary>
          {msg.content && (
            <FoldSection
              label="Content"
              preview={formatConversationPreview(unescapeLineBreaks(msg.content), 88)}
              defaultOpen={isLive}
            >
              <div className="acp-text">
                <pre>{unescapeLineBreaks(msg.content)}</pre>
              </div>
            </FoldSection>
          )}
          {msg.raw_input && (
            <FoldSection
              label="Input"
              preview={formatConversationPreview(formatToolCallPayload(msg.raw_input), 88)}
              defaultOpen={false}
            >
              <pre className="acp-content">
                {formatToolCallPayload(msg.raw_input)}
              </pre>
            </FoldSection>
          )}
          {msg.raw_output && (
            <FoldSection
              label="Output"
              preview={formatConversationPreview(formatToolCallPayload(msg.raw_output), 88)}
              defaultOpen={!isLive}
            >
              <pre className="acp-content">
                {formatToolCallPayload(msg.raw_output)}
              </pre>
            </FoldSection>
          )}
          {msg.terminal_output && (
            <FoldSection
              label="Terminal"
              preview={formatConversationPreview(unescapeLineBreaks(msg.terminal_output), 88)}
              defaultOpen={isLive}
            >
              <pre className="acp-content">
                {renderAnsiTerminalOutput(
                  ansi(unescapeLineBreaks(msg.terminal_output))
                )}
              </pre>
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
  children: React.ReactNode;
};

function FoldSection({ label, preview, defaultOpen, children }: FoldSectionProps) {
  return (
    <details className="acp-subfold" open={defaultOpen}>
      <summary>
        <span>{label}</span>
        {preview ? <span className="acp-subfold-preview">{preview}</span> : null}
      </summary>
      {children}
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

function formatToolCallPayload(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return unescapeLineBreaks(value);
  return unescapeLineBreaks(JSON.stringify(value, null, 2));
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
