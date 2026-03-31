import { layout, prepare, type PreparedText } from "@chenglou/pretext";
import type { ConversationItem } from "../conversation";

export type ConversationHeightEstimateModel = {
  heights: number[];
  offsets: number[];
  totalHeight: number;
};

const DEFAULT_VIEWPORT_WIDTH = 720;
const MIN_CONTENT_WIDTH = 180;
const DEFAULT_ITEM_HEIGHT = 48;
const MIN_ITEM_HEIGHT = 24;
const MAX_ITEM_HEIGHT = 3000;
const MESSAGE_FONT =
  '400 14px "Space Grotesk", system-ui, sans-serif';
const MESSAGE_LINE_HEIGHT = 24;
const MESSAGE_MAX_WIDTH_RATIO = 0.95;
const MESSAGE_HORIZONTAL_CHROME = 26;
const MESSAGE_VERTICAL_CHROME = 18;
const MESSAGE_MIN_HEIGHT = 42;
const MESSAGE_PARAGRAPH_GAP = 10;
const CODE_BLOCK_VERTICAL_CHROME = 36;
const CODE_BLOCK_LINE_HEIGHT = 20;
const PREPARED_CACHE_LIMIT = 512;
const HEIGHT_CACHE_LIMIT = 1024;

const markdownLinkPattern = /\[([^\]]+)\]\(([^)]+)\)/g;
const markdownImagePattern = /!\[([^\]]*)\]\(([^)]+)\)/g;
const markdownInlineCodePattern = /`([^`]+)`/g;
const markdownDecorationPattern = /[*_~>#]+/g;

const preparedTextCache = new Map<string, PreparedText>();
const estimatedHeightCache = new Map<string, number>();
let canMeasureRichText: boolean | null = null;

export function resetConversationHeightEstimateCaches(): void {
  preparedTextCache.clear();
  estimatedHeightCache.clear();
  canMeasureRichText = null;
}

export function buildConversationHeightEstimateModel(
  items: ConversationItem[],
  viewportWidth: number,
  fallbackHeight: number
): ConversationHeightEstimateModel {
  const heights = new Array<number>(items.length);
  const offsets = new Array<number>(items.length + 1);
  offsets[0] = 0;
  for (let index = 0; index < items.length; index += 1) {
    const height = estimateConversationItemHeight(
      items[index],
      viewportWidth,
      fallbackHeight
    );
    heights[index] = height;
    offsets[index + 1] = offsets[index] + height;
  }
  return {
    heights,
    offsets,
    totalHeight: offsets[offsets.length - 1] ?? 0,
  };
}

export function estimateConversationItemHeight(
  item: ConversationItem,
  viewportWidth: number,
  fallbackHeight: number
): number {
  const fallback = normalizeFallbackHeight(fallbackHeight);
  if (item.kind !== "agent_message" && item.kind !== "user_message") {
    return fallback;
  }
  return estimateMarkdownBubbleHeight(item.text, viewportWidth, fallback);
}

export function estimateMarkdownBubbleHeight(
  text: string,
  viewportWidth: number,
  fallbackHeight: number
): number {
  const estimateInput = buildHeightEstimateInput(text);
  const { normalizedText, codeBlockCount, codeBlockLineCount } = estimateInput;
  const contentWidth = resolveMessageContentWidth(viewportWidth);
  const cacheKey = `${contentWidth}:${fallbackHeight}:${codeBlockCount}:${codeBlockLineCount}:${normalizedText}`;
  const cached = estimatedHeightCache.get(cacheKey);
  if (cached != null) {
    refreshCacheRecency(estimatedHeightCache, cacheKey);
    return cached;
  }

  if (!canUseRichTextMeasurement()) {
    const fallback = normalizeEstimatedHeight(
      Math.max(MESSAGE_MIN_HEIGHT, fallbackHeight)
    );
    cacheWithLimit(estimatedHeightCache, cacheKey, fallback, HEIGHT_CACHE_LIMIT);
    return fallback;
  }

  const textHeight = normalizedText
    ? measureRichTextHeight(normalizedText, contentWidth)
    : 0;
  const codeBlockHeight = estimateCodeBlockHeight(
    codeBlockCount,
    codeBlockLineCount
  );
  const measured =
    textHeight > 0 || codeBlockHeight > 0
      ? MESSAGE_VERTICAL_CHROME + textHeight + codeBlockHeight
      : fallbackHeight;
  const height = normalizeEstimatedHeight(
    Math.max(MESSAGE_MIN_HEIGHT, measured)
  );
  cacheWithLimit(estimatedHeightCache, cacheKey, height, HEIGHT_CACHE_LIMIT);
  return height;
}

export function buildVirtualConversationSliceWithHeightModel(
  viewportTop: number,
  viewportHeight: number,
  model: ConversationHeightEstimateModel,
  overscan: number
): {
  start: number;
  end: number;
  topSpacer: number;
  bottomSpacer: number;
} {
  const totalItems = Math.max(0, model.offsets.length - 1);
  if (totalItems === 0) {
    return {
      start: 0,
      end: 0,
      topSpacer: 0,
      bottomSpacer: 0,
    };
  }

  const safeHeight = Math.max(1, viewportHeight);
  const maxViewportTop = Math.max(0, model.totalHeight - safeHeight);
  const clampedTop = Math.min(Math.max(0, viewportTop), maxViewportTop);
  const clampedBottom = clampedTop + safeHeight;
  const startIndex = Math.max(
    0,
    upperBound(model.offsets, clampedTop) - 1 - overscan
  );
  const endIndex = Math.min(
    totalItems,
    Math.max(
      startIndex + 1,
      lowerBound(model.offsets, clampedBottom) + overscan
    )
  );

  return {
    start: startIndex,
    end: endIndex,
    topSpacer: Math.max(0, Math.round(model.offsets[startIndex] ?? 0)),
    bottomSpacer: Math.max(
      0,
      Math.round(model.totalHeight - (model.offsets[endIndex] ?? model.totalHeight))
    ),
  };
}

function normalizeTextForHeightEstimate(text: string): string {
  return text
    .replace(/\r\n?/g, "\n")
    .replace(markdownImagePattern, "$1")
    .replace(markdownLinkPattern, "$1")
    .replace(markdownInlineCodePattern, "$1")
    .replace(markdownDecorationPattern, "")
    .trim();
}

function buildHeightEstimateInput(text: string): {
  normalizedText: string;
  codeBlockCount: number;
  codeBlockLineCount: number;
} {
  let codeBlockCount = 0;
  let codeBlockLineCount = 0;
  const withoutCodeBlocks = text.replace(
    /```[\w-]*\n?([\s\S]*?)```/g,
    (_, rawBody: string) => {
      codeBlockCount += 1;
      const normalizedBody = rawBody.replace(/\r\n?/g, "\n").trim();
      codeBlockLineCount += normalizedBody
        ? normalizedBody.split("\n").length
        : 1;
      return "\n\n";
    }
  );
  return {
    normalizedText: normalizeTextForHeightEstimate(withoutCodeBlocks),
    codeBlockCount,
    codeBlockLineCount,
  };
}

function canUseRichTextMeasurement(): boolean {
  if (canMeasureRichText != null) {
    return canMeasureRichText;
  }
  try {
    const probe = prepare("AgentHub", MESSAGE_FONT);
    const result = layout(probe, 120, MESSAGE_LINE_HEIGHT);
    canMeasureRichText = Number.isFinite(result.height) && result.height > 0;
  } catch {
    canMeasureRichText = false;
  }
  return canMeasureRichText;
}

function measureRichTextHeight(text: string, maxWidth: number): number {
  const prepared = getPreparedTextCached(text);
  const paragraphCount = text.split(/\n\s*\n/).filter(Boolean).length;
  const result = layout(prepared, maxWidth, MESSAGE_LINE_HEIGHT);
  const paragraphGap =
    paragraphCount > 1 ? (paragraphCount - 1) * MESSAGE_PARAGRAPH_GAP : 0;
  return result.height + paragraphGap;
}

function getPreparedTextCached(text: string): PreparedText {
  const cached = preparedTextCache.get(text);
  if (cached != null) {
    refreshCacheRecency(preparedTextCache, text);
    return cached;
  }
  const prepared = prepare(text, MESSAGE_FONT);
  cacheWithLimit(preparedTextCache, text, prepared, PREPARED_CACHE_LIMIT);
  return prepared;
}

function resolveMessageContentWidth(viewportWidth: number): number {
  const containerWidth =
    Number.isFinite(viewportWidth) && viewportWidth > 0
      ? viewportWidth
      : DEFAULT_VIEWPORT_WIDTH;
  return Math.max(
    MIN_CONTENT_WIDTH,
    Math.floor(containerWidth * MESSAGE_MAX_WIDTH_RATIO) - MESSAGE_HORIZONTAL_CHROME
  );
}

function estimateCodeBlockHeight(
  codeBlockCount: number,
  codeBlockLineCount: number
): number {
  if (codeBlockCount <= 0 || codeBlockLineCount <= 0) {
    return 0;
  }
  return (
    codeBlockLineCount * CODE_BLOCK_LINE_HEIGHT +
    codeBlockCount * CODE_BLOCK_VERTICAL_CHROME
  );
}

function normalizeFallbackHeight(height: number): number {
  if (!Number.isFinite(height) || height <= 0) {
    return DEFAULT_ITEM_HEIGHT;
  }
  return normalizeEstimatedHeight(height);
}

function normalizeEstimatedHeight(height: number): number {
  return Math.min(MAX_ITEM_HEIGHT, Math.max(MIN_ITEM_HEIGHT, Math.round(height)));
}

function lowerBound(values: number[], target: number): number {
  let low = 0;
  let high = values.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if ((values[mid] ?? 0) < target) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  return low;
}

function upperBound(values: number[], target: number): number {
  let low = 0;
  let high = values.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if ((values[mid] ?? 0) <= target) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  return low;
}

function cacheWithLimit<K, V>(
  cache: Map<K, V>,
  key: K,
  value: V,
  limit: number
): void {
  if (cache.has(key)) {
    cache.delete(key);
  }
  cache.set(key, value);
  while (cache.size > limit) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey === undefined) {
      break;
    }
    cache.delete(oldestKey);
  }
}

function refreshCacheRecency<K, V>(cache: Map<K, V>, key: K): void {
  const value = cache.get(key);
  if (value == null) {
    return;
  }
  cache.delete(key);
  cache.set(key, value);
}
