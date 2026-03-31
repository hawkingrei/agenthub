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
const IMAGE_BLOCK_ESTIMATE = 160;
const CODE_BLOCK_VERTICAL_CHROME = 36;
const CODE_BLOCK_LINE_HEIGHT = 20;
const BLOCKQUOTE_BLOCK_VERTICAL_CHROME = 12;
const PREPARED_CACHE_LIMIT = 512;
const HEIGHT_CACHE_LIMIT = 1024;
const H1_FONT = '600 18px "Space Grotesk", system-ui, sans-serif';
const H1_LINE_HEIGHT = 22.5;
const H2_FONT = '600 16px "Space Grotesk", system-ui, sans-serif';
const H2_LINE_HEIGHT = 20.8;
const H3_FONT = '600 15px "Space Grotesk", system-ui, sans-serif';
const H3_LINE_HEIGHT = 20.25;
const H4_FONT = '600 14px "Space Grotesk", system-ui, sans-serif';
const H4_LINE_HEIGHT = 19.6;

const markdownLinkPattern = /\[([^\]]+)\]\(([^)]+)\)/g;
const markdownImagePattern = /!\[([^\]]*)\]\(([^)]+)\)/g;
const markdownInlineCodePattern = /`([^`]+)`/g;
const markdownDecorationPattern = /[*_~]+/g;

const preparedTextCache = new Map<string, PreparedText>();
const fallbackHeightCache = new Map<string, number>();
const measuredHeightCache = new Map<string, Map<string, number>>();
let canMeasureRichText: boolean | null = null;

export function resetConversationHeightEstimateCaches(): void {
  preparedTextCache.clear();
  fallbackHeightCache.clear();
  measuredHeightCache.clear();
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
  const {
    normalizedText,
    codeBlockCount,
    codeBlockLineCount,
    blockquoteBlockCount,
    headingBlocks,
    imageCount,
  } = estimateInput;
  const contentWidth = resolveMessageContentWidth(viewportWidth);
  if (!canUseRichTextMeasurement()) {
    const fallbackCacheKey = `fallback:${fallbackHeight}`;
    const cachedFallback = fallbackHeightCache.get(fallbackCacheKey);
    if (cachedFallback != null) {
      refreshCacheRecency(fallbackHeightCache, fallbackCacheKey);
      return cachedFallback;
    }
    const fallback = normalizeEstimatedHeight(
      Math.max(MESSAGE_MIN_HEIGHT, fallbackHeight)
    );
    cacheWithLimit(
      fallbackHeightCache,
      fallbackCacheKey,
      fallback,
      HEIGHT_CACHE_LIMIT
    );
    return fallback;
  }

  const structureKey = [
    contentWidth,
    codeBlockCount,
    codeBlockLineCount,
    blockquoteBlockCount,
    imageCount,
    ...headingBlocks.flatMap((heading) => [heading.level, heading.text]),
  ].join(":");
  const cached = getMeasuredHeightCache(
    normalizedText,
    structureKey
  );
  if (cached != null) {
    return cached;
  }

  const textHeight = normalizedText
    ? measureRichTextHeight(normalizedText, contentWidth)
    : 0;
  const codeBlockHeight = estimateCodeBlockHeight(
    codeBlockCount,
    codeBlockLineCount
  );
  const headingHeight = estimateHeadingBlockHeight(headingBlocks, contentWidth);
  const markdownStructureHeight = estimateMarkdownStructureHeight(
    blockquoteBlockCount,
    imageCount
  );
  const measured =
    textHeight > 0 ||
    codeBlockHeight > 0 ||
    headingHeight > 0 ||
    markdownStructureHeight > 0
      ? MESSAGE_VERTICAL_CHROME +
        textHeight +
        codeBlockHeight +
        headingHeight +
        markdownStructureHeight
      : fallbackHeight;
  const height = normalizeEstimatedHeight(
    Math.max(MESSAGE_MIN_HEIGHT, measured)
  );
  setMeasuredHeightCache(
    normalizedText,
    structureKey,
    height
  );
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
  const normalized = text
    .replace(/\r\n?/g, "\n")
    .replace(markdownImagePattern, "$1")
    .replace(markdownLinkPattern, "$1")
    .replace(markdownInlineCodePattern, "$1")
    .split("\n")
    .map((line) =>
      line
        .replace(/^\s{0,3}#{1,6}\s+/, "")
        .replace(/^\s*>+\s?/, "")
    )
    .join("\n")
    .replace(markdownDecorationPattern, "")
    .trim();
  return normalized;
}

function buildHeightEstimateInput(text: string): {
  normalizedText: string;
  codeBlockCount: number;
  codeBlockLineCount: number;
  blockquoteBlockCount: number;
  headingBlocks: { level: 1 | 2 | 3 | 4; text: string }[];
  imageCount: number;
} {
  let imageCount = 0;
  const withoutImages = text.replace(markdownImagePattern, (_full, altText: string) => {
    imageCount += 1;
    return altText ? `${altText}\n\n` : "\n\n";
  });
  let codeBlockCount = 0;
  let codeBlockLineCount = 0;
  const withoutCodeBlocks = withoutImages.replace(
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
  let blockquoteBlockCount = 0;
  let inBlockquote = false;
  const headingBlocks: { level: 1 | 2 | 3 | 4; text: string }[] = [];
  const proseLines: string[] = [];
  for (const rawLine of withoutCodeBlocks.split("\n")) {
    const trimmedLine = rawLine.trim();
    const isBlockquote = /^\s*> ?/.test(rawLine);
    if (isBlockquote && !inBlockquote) {
      blockquoteBlockCount += 1;
    }
    inBlockquote = isBlockquote;
    const headingMatch = trimmedLine.match(/^(#{1,4})\s+\S/);
    if (headingMatch) {
      const level = headingMatch[1].length;
      headingBlocks.push({
        level: level as 1 | 2 | 3 | 4,
        text: trimmedLine.slice(level).trim(),
      });
      proseLines.push("");
      continue;
    }
    proseLines.push(rawLine);
  }
  return {
    normalizedText: normalizeTextForHeightEstimate(proseLines.join("\n")),
    codeBlockCount,
    codeBlockLineCount,
    blockquoteBlockCount,
    headingBlocks,
    imageCount,
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
  const prepared = getPreparedTextCached(text, MESSAGE_FONT);
  const paragraphCount = text.split(/\n\s*\n/).filter(Boolean).length;
  const result = layout(prepared, maxWidth, MESSAGE_LINE_HEIGHT);
  const paragraphGap =
    paragraphCount > 1 ? (paragraphCount - 1) * MESSAGE_PARAGRAPH_GAP : 0;
  return result.height + paragraphGap;
}

function getPreparedTextCached(text: string, font: string): PreparedText {
  const cacheKey = `${font}:${text}`;
  const cached = preparedTextCache.get(cacheKey);
  if (cached != null) {
    refreshCacheRecency(preparedTextCache, cacheKey);
    return cached;
  }
  const prepared = prepare(text, font);
  cacheWithLimit(preparedTextCache, cacheKey, prepared, PREPARED_CACHE_LIMIT);
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

function estimateHeadingBlockHeight(
  headingBlocks: readonly { level: 1 | 2 | 3 | 4; text: string }[],
  contentWidth: number
): number {
  if (headingBlocks.length === 0) {
    return 0;
  }
  return headingBlocks.reduce((total, heading) => {
    const prepared = getPreparedTextCached(
      heading.text,
      headingFontForLevel(heading.level)
    );
    const result = layout(
      prepared,
      contentWidth,
      headingLineHeightForLevel(heading.level)
    );
    return total + result.height + MESSAGE_PARAGRAPH_GAP;
  }, 0);
}

function estimateMarkdownStructureHeight(
  blockquoteBlockCount: number,
  imageCount: number
): number {
  return (
    blockquoteBlockCount * BLOCKQUOTE_BLOCK_VERTICAL_CHROME +
    imageCount * (IMAGE_BLOCK_ESTIMATE + MESSAGE_PARAGRAPH_GAP)
  );
}

function getMeasuredHeightCache(
  normalizedText: string,
  structureKey: string
): number | undefined {
  const byStructure = measuredHeightCache.get(normalizedText);
  if (!byStructure) {
    return undefined;
  }
  refreshCacheRecency(measuredHeightCache, normalizedText);
  const cached = byStructure.get(structureKey);
  if (cached == null) {
    return undefined;
  }
  refreshCacheRecency(byStructure, structureKey);
  return cached;
}

function setMeasuredHeightCache(
  normalizedText: string,
  structureKey: string,
  height: number
): void {
  let byStructure = measuredHeightCache.get(normalizedText);
  if (!byStructure) {
    byStructure = new Map<string, number>();
    cacheWithLimit(
      measuredHeightCache,
      normalizedText,
      byStructure,
      HEIGHT_CACHE_LIMIT
    );
  } else {
    refreshCacheRecency(measuredHeightCache, normalizedText);
  }
  if (byStructure.has(structureKey)) {
    byStructure.delete(structureKey);
  }
  byStructure.set(structureKey, height);
}

function headingFontForLevel(level: 1 | 2 | 3 | 4): string {
  switch (level) {
    case 1:
      return H1_FONT;
    case 2:
      return H2_FONT;
    case 3:
      return H3_FONT;
    case 4:
      return H4_FONT;
  }
}

function headingLineHeightForLevel(level: 1 | 2 | 3 | 4): number {
  switch (level) {
    case 1:
      return H1_LINE_HEIGHT;
    case 2:
      return H2_LINE_HEIGHT;
    case 3:
      return H3_LINE_HEIGHT;
    case 4:
      return H4_LINE_HEIGHT;
  }
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
