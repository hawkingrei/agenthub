import React from "react";
import { unescapeLineBreaks } from "../conversation";
import { cacheWithLruBudget, refreshCacheRecency } from "../cache_with_lru_budget";
import { ACP_TERMINAL_PRE_CLASS } from "../ui/tailwind_classes";
import {
  ACP_SEGMENTED_BLOCK_CLASS,
  TOOL_TEXT_INITIAL_LINES,
  TOOL_TEXT_LINE_CHUNK,
  isJsonLikeText,
  isPlainObject,
  parseJsonLikeString,
  registerPayloadParseCounters,
  type NormalizedToolPayload,
} from "./acp_tool_payload_content";
import {
  SegmentedMoreFooter,
  useProgressiveTailWindow,
} from "./acp_progressive_views";

export {
  TOOL_PAYLOAD_PREVIEW_LIMIT,
  ToolCallDetailsView,
  ToolPayloadView,
  ToolTextContent,
  summarizeToolPayload,
  shouldAutoExpandToolContent,
  type NormalizedToolPayload,
} from "./acp_tool_payload_content";

const ANSI_SEGMENT_CACHE_LIMIT = 512;
const ANSI_SEGMENT_CACHE_MAX_BYTES = 4 * 1024 * 1024;
const ANSI_SEGMENT_CACHE_MAX_ENTRY_CHARS = 120_000;
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

const ansiSegmentCache = new Map<string, AnsiSegment[]>();
const ansiSegmentCacheSize = new Map<string, number>();
let ansiCacheBytes = 0;
let ansiCacheHitCount = 0;
let ansiCacheMissCount = 0;
let payloadParseCount = 0;
let payloadParseFailureCount = 0;

registerPayloadParseCounters({
  onParse: () => {
    payloadParseCount += 1;
  },
  onParseFailure: () => {
    payloadParseFailureCount += 1;
  },
});

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

export const TerminalOutputView = React.memo(function TerminalOutputView({
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
});

export function parseAnsiSegmentsCached(input: string): AnsiSegment[] {
  if (input.length > ANSI_SEGMENT_CACHE_MAX_ENTRY_CHARS) {
    ansiCacheMissCount += 1;
    return parseAnsiSegments(input);
  }
  const cached = ansiSegmentCache.get(input);
  if (cached != null) {
    ansiCacheHitCount += 1;
    refreshCacheRecency(ansiSegmentCache, ansiSegmentCacheSize, input);
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
