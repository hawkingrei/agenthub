import React from "react";
import {
  formatConversationPreview,
  unescapeLineBreaks,
} from "../conversation";
import {
  ACP_DIFF_PRE_CLASS,
  ACP_PAYLOAD_MARKDOWN_CLASS,
  ACP_SEGMENTED_NOTE_WARNING_CLASS,
  ACP_TERMINAL_PRE_CLASS,
} from "../ui/tailwind_classes";
import {
  SegmentedMoreFooter,
  useProgressiveTailWindow,
  useProgressiveVisibleCount,
} from "./acp_progressive_views";
import type { ToolCallDetailItem } from "./acp_tool_call_meta";
import { ThreadRichText } from "./thread_rich_text";

export const TOOL_PAYLOAD_PREVIEW_LIMIT = 64;
export const TOOL_TEXT_INITIAL_LINES = 36;
export const TOOL_TEXT_LINE_CHUNK = 120;
export const ACP_SEGMENTED_BLOCK_CLASS = "acp-segmented-block grid gap-1.5";

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
const TOOL_TEXT_MARKDOWN_FALLBACK_LINES = 260;
const TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH = 16000;
const TOOL_PAYLOAD_INITIAL_ITEMS = 8;
const TOOL_PAYLOAD_ITEM_CHUNK = 16;

const ACP_PAYLOAD_CARD_CLASS =
  "acp-payload-card overflow-hidden rounded-[10px] border border-notion-payload-border bg-notion-payload-bg px-[7px] py-1.5 shadow-[0_1px_0_rgba(15,23,42,0.03)] max-[720px]:rounded-lg max-[720px]:px-1.5 max-[720px]:py-[5px]";
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
  "acp-content-markdown rounded-lg border border-notion-border bg-notion-code-bg px-4 py-3 text-[13px] leading-relaxed text-slate-200 shadow-inner [&_.hljs]:bg-transparent [&_a]:text-sky-300 [&_blockquote]:border-l-2 [&_blockquote]:border-white/15 [&_blockquote]:pl-3 [&_blockquote]:text-slate-300 [&_code]:rounded [&_code]:bg-white/10 [&_code]:px-1 [&_code]:text-slate-100 [&_li]:marker:text-slate-500 [&_p]:mb-3 [&_p:last-child]:mb-0 [&_pre]:my-3 [&_pre]:border-0 [&_pre]:bg-transparent [&_pre]:p-0 [&_pre]:text-inherit [&_strong]:text-white";
const ACP_PAYLOAD_SEGMENTED_CLASS = "acp-payload-segmented grid gap-1.5";
const ACP_PAYLOAD_LIST_CLASS = "acp-payload-list m-0 grid list-none gap-1.5 pl-0";
const ACP_PAYLOAD_LIST_ITEM_CLASS = "m-0 min-w-0";
const ACP_PAYLOAD_NESTED_CLASS = "acp-payload-nested rounded-md border border-slate-200 bg-white";
const ACP_PAYLOAD_NESTED_SUMMARY_CLASS =
  "cursor-pointer font-mono text-[10.5px] leading-[1.35] text-slate-500";
const ACP_PAYLOAD_NESTED_BODY_CLASS =
  "acp-payload-nested-body mt-1 border-t border-slate-200 px-2 py-2";

export type NormalizedToolPayload =
  | { kind: "empty" }
  | { kind: "text"; text: string }
  | { kind: "json_text"; text: string; parsed?: unknown }
  | { kind: "json"; value: unknown };

type DiffLineKind = "meta" | "hunk" | "add" | "remove" | "context";

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

export const ToolPayloadView = React.memo(function ToolPayloadView({
  payload,
}: {
  payload: NormalizedToolPayload;
}) {
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
});

export const ToolCallDetailsView = React.memo(function ToolCallDetailsView({
  details,
}: {
  details: ToolCallDetailItem[];
}) {
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
});

export const ToolTextContent = React.memo(function ToolTextContent({
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
});

export function shouldAutoExpandToolContent(text: string): boolean {
  if (!text) return false;
  if (!shouldRenderMarkdownText(text)) return false;
  if (shouldRenderDiffText(text)) return false;
  if (countLines(text) > TOOL_TEXT_MARKDOWN_FALLBACK_LINES) return false;
  if (text.length > TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH) return false;
  return true;
}

export function isJsonLikeText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

export function parseJsonLikeString(value: string, countParse = false): unknown | undefined {
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return undefined;
  if (countParse) {
    incrementPayloadParseCount();
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    if (countParse) {
      incrementPayloadParseFailureCount();
    }
    return undefined;
  }
}

export function isPlainObject(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value)
  );
}

let payloadParseCountSink: (() => void) | null = null;
let payloadParseFailureCountSink: (() => void) | null = null;

export function registerPayloadParseCounters(options: {
  onParse: () => void;
  onParseFailure: () => void;
}): void {
  payloadParseCountSink = options.onParse;
  payloadParseFailureCountSink = options.onParseFailure;
}

function incrementPayloadParseCount(): void {
  payloadParseCountSink?.();
}

function incrementPayloadParseFailureCount(): void {
  payloadParseFailureCountSink?.();
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

function findPreferredOutputPayloadKey(entries: Array<[string, unknown]>): string | null {
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

function filterPayloadEntries(value: Record<string, unknown>): Array<[string, unknown]> {
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
