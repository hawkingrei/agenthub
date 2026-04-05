import React from "react";
import {
  formatConversationPreview,
  unescapeLineBreaks,
} from "../conversation";
import {
  ACP_DIFF_PRE_CLASS,
  ACP_PAYLOAD_MARKDOWN_CLASS,
  ACP_SEGMENTED_BUTTON_CLASS,
  ACP_SEGMENTED_NOTE_WARNING_CLASS,
  ACP_TERMINAL_PRE_CLASS,
} from "../ui/tailwind_classes";
import type { ToolCallDetailItem } from "./acp_tool_call_meta";
import { ThreadRichText } from "./thread_rich_text";

export const TOOL_PAYLOAD_PREVIEW_LIMIT = 64;

const ANSI_SEGMENT_CACHE_LIMIT = 512;
const ANSI_SEGMENT_CACHE_MAX_BYTES = 4 * 1024 * 1024;
const ANSI_SEGMENT_CACHE_MAX_ENTRY_CHARS = 120_000;
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

export type ToolContentCacheStats = {
  ansiHits: number;
  ansiMisses: number;
  payloadParses: number;
  payloadParseFailures: number;
};

export type NormalizedToolPayload =
  | { kind: "empty" }
  | { kind: "text"; text: string }
  | { kind: "json_text"; text: string; parsed?: unknown }
  | { kind: "json"; value: unknown };

type DiffLineKind = "meta" | "hunk" | "add" | "remove" | "context";

const ansiSegmentCache = new Map<string, AnsiSegment[]>();
const ansiSegmentCacheSize = new Map<string, number>();
let ansiCacheBytes = 0;
let ansiCacheHitCount = 0;
let ansiCacheMissCount = 0;
let payloadParseCount = 0;
let payloadParseFailureCount = 0;

export function resetToolContentCaches(): void {
  ansiSegmentCache.clear();
  ansiSegmentCacheSize.clear();
  ansiCacheBytes = 0;
  ansiCacheHitCount = 0;
  ansiCacheMissCount = 0;
  payloadParseCount = 0;
  payloadParseFailureCount = 0;
}

export function getToolContentCacheStats(): ToolContentCacheStats {
  return {
    ansiHits: ansiCacheHitCount,
    ansiMisses: ansiCacheMissCount,
    payloadParses: payloadParseCount,
    payloadParseFailures: payloadParseFailureCount,
  };
}

export function normalizeToolPayload(value: unknown): NormalizedToolPayload {
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
  if (isPlainObject(value) || Array.isArray(value)) {
    return { kind: "json", value };
  }
  return {
    kind: "text",
    text: String(value),
  };
}

export function hasToolPayload(payload: NormalizedToolPayload): boolean {
  return payload.kind !== "empty";
}

export function summarizeToolPayload(payload: NormalizedToolPayload, limit: number): string {
  if (payload.kind === "empty") return "";
  if (payload.kind === "text") return formatConversationPreview(payload.text, limit);
  if (payload.kind === "json_text") {
    if (payload.parsed !== undefined) {
      return formatConversationPreview(summarizePayloadValue(payload.parsed), limit);
    }
    return formatConversationPreview(payload.text, limit);
  }
  return formatConversationPreview(summarizePayloadValue(payload.value), limit);
}

export function ToolPayloadView({ payload }: { payload: NormalizedToolPayload }) {
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

export function ToolCallDetailsView({ details }: { details: ToolCallDetailItem[] }) {
  return (
    <div className={ACP_PAYLOAD_CARD_CLASS}>
      <dl className={ACP_PAYLOAD_GRID_CLASS}>
        {details.map((detail) => (
          <div className={ACP_PAYLOAD_ROW_CLASS} key={detail.key}>
            <dt>{detail.key}</dt>
            <dd className="font-medium text-sm text-notion-text opacity-90">
              <code>{detail.value}</code>
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export function ToolTextContent({
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
  const resolvedMarkdownClassName =
    markdownClassName ??
    (tone === "terminal" ? ACP_CONTENT_MARKDOWN_CLASS : ACP_PAYLOAD_MARKDOWN_CLASS);

  if (markdownText && !tooLargeForMarkdown) {
    return <ThreadRichText text={text} className={resolvedMarkdownClassName} />;
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

export function shouldAutoExpandToolContent(text: string): boolean {
  if (!text) return false;
  if (!shouldRenderMarkdownText(text)) return false;
  if (shouldRenderDiffText(text)) return false;
  if (countLines(text) > TOOL_TEXT_MARKDOWN_FALLBACK_LINES) return false;
  if (text.length > TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH) return false;
  return true;
}

export function TerminalOutputView({
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

function normalizePayloadKey(key: string): string {
  return key.trim().toLowerCase().replace(/[\s_-]+/g, "");
}

function isHiddenPayloadKey(key: string): boolean {
  return TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function isPayloadValueEffectivelyEmpty(value: unknown): boolean {
  if (value == null) return true;
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
    const match = entries.find(
      ([key, value]) =>
        normalizePayloadKey(key) === normalizedKey &&
        !isPayloadValueEffectivelyEmpty(value)
    );
    if (match) return match[0];
  }
  return null;
}

function filterPayloadEntries(
  value: Record<string, unknown>
): Array<[string, unknown]> {
  const entries = Object.entries(value);
  const preferredOutputKey = findPreferredOutputPayloadKey(entries);
  return entries.filter(([key, item]) => {
    const normalized = normalizePayloadKey(key);
    if (
      TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED.has(normalized) &&
      key !== preferredOutputKey
    ) {
      return false;
    }
    return !isHiddenPayloadKey(key) && !isEmptyStdStreamPayloadField(key, item);
  });
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
          <SegmentedMoreFooter remaining={remaining} unitLabel="items" onShowMore={showMore} />
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
        <SegmentedMoreFooter remaining={remaining} unitLabel="items" onShowMore={showMore} />
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
            <dd className="font-medium text-notion-text text-sm opacity-90">
              {renderPayloadFieldValue(key, item, depth + 1)}
            </dd>
          </div>
        ))}
      </dl>
      {hasMore && (
        <SegmentedMoreFooter remaining={remaining} unitLabel="fields" onShowMore={showMore} />
      )}
    </div>
  );
}

function renderPayloadFieldValue(
  key: string,
  value: unknown,
  depth: number
): React.ReactNode {
  if (typeof value === "string" && shouldPreferPlainTextForPayloadKey(key)) {
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
    return <div className="acp-payload-inline">{renderPayloadValue(value, depth)}</div>;
  }
  if (isStructured) {
    return (
      <details className={ACP_PAYLOAD_NESTED_CLASS}>
        <summary
          className={`${ACP_PAYLOAD_NESTED_SUMMARY_CLASS} flex cursor-pointer list-none items-center gap-2 px-2 py-1.5`}
        >
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
        <SegmentedMoreFooter remaining={remaining} unitLabel="lines" onShowMore={showMore} />
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
      return "bg-sky-400/15 text-sky-200";
    case "hunk":
      return "bg-violet-400/15 text-violet-200";
    case "add":
      return "bg-emerald-400/15 text-emerald-200";
    case "remove":
      return "bg-rose-400/15 text-rose-200";
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
        <SegmentedMoreFooter remaining={remaining} unitLabel="lines" onShowMore={showMore} />
      )}
    </div>
  );
}

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

function estimateAnsiSegmentsBytes(input: string, segments: AnsiSegment[]): number {
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

function isJsonLikeText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

function parseJsonLikeString(value: string, countParse = false): unknown | undefined {
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return undefined;
  if (countParse) {
    payloadParseCount += 1;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    if (countParse) {
      payloadParseFailureCount += 1;
    }
    return undefined;
  }
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value)
  );
}
