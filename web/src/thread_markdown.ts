import { renderMarkdown } from "./markdown";
import { cacheWithLruBudget, refreshCacheRecency } from "./cache_with_lru_budget";

const MARKDOWN_CACHE_LIMIT = 512;
const MARKDOWN_CACHE_MAX_BYTES = 8 * 1024 * 1024;
const MARKDOWN_CACHE_MAX_ENTRY_CHARS = 120_000;
const SKILL_BLOCK_PATTERN = /<skill>\s*([\s\S]*?)\s*<\/skill>/gi;

export const THREAD_RICH_TEXT_BASE_CLASS =
  "acp-text min-w-0 max-w-full text-sm leading-6 [overflow-wrap:anywhere] [&_code]:break-words [&_li]:break-words [&_ol]:max-w-full [&_p]:break-words [&_pre]:max-w-full [&_pre]:whitespace-pre-wrap [&_pre]:break-words [&_pre_code]:whitespace-pre-wrap [&_pre_code]:break-words [&_table]:block [&_table]:max-w-full [&_table]:overflow-x-auto [&_td]:break-words [&_th]:break-words [&_ul]:max-w-full";

const markdownHtmlCache = new Map<string, string>();
const markdownHtmlCacheSize = new Map<string, number>();
let markdownCacheBytes = 0;
let markdownCacheHitCount = 0;
let markdownCacheMissCount = 0;

export type ThreadMarkdownCacheStats = {
  markdownHits: number;
  markdownMisses: number;
};

export function resetThreadMarkdownCache(): void {
  markdownHtmlCache.clear();
  markdownHtmlCacheSize.clear();
  markdownCacheBytes = 0;
  markdownCacheHitCount = 0;
  markdownCacheMissCount = 0;
}

export function getThreadMarkdownCacheStats(): ThreadMarkdownCacheStats {
  return {
    markdownHits: markdownCacheHitCount,
    markdownMisses: markdownCacheMissCount,
  };
}

export function renderThreadMarkdownCached(text: string): string {
  const normalized = normalizeSkillBlocksForMarkdown(text);
  if (text.length > MARKDOWN_CACHE_MAX_ENTRY_CHARS) {
    markdownCacheMissCount += 1;
    return renderMarkdown(normalized);
  }
  const cached = markdownHtmlCache.get(text);
  if (cached != null) {
    markdownCacheHitCount += 1;
    refreshCacheRecency(markdownHtmlCache, markdownHtmlCacheSize, text);
    return cached;
  }
  markdownCacheMissCount += 1;
  const rendered = renderMarkdown(normalized);
  const estimatedBytes = estimateStringBytes(text) + estimateStringBytes(rendered);
  return cacheWithLruBudget(
    markdownHtmlCache,
    markdownHtmlCacheSize,
    () => markdownCacheBytes,
    (next) => {
      markdownCacheBytes = next;
    },
    text,
    rendered,
    estimatedBytes,
    MARKDOWN_CACHE_LIMIT,
    MARKDOWN_CACHE_MAX_BYTES
  );
}

export async function preloadThreadMarkdownAssets(): Promise<void> {
  return Promise.resolve();
}

function normalizeSkillBlocksForMarkdown(text: string): string {
  return text.replace(SKILL_BLOCK_PATTERN, (block) => {
    const name = extractSkillField(block, "name");
    const path = extractSkillField(block, "path");
    if (!name && !path) return block;
    const lines = ["**Skill**"];
    if (name) lines.push(`- Name: ${formatInlineCode(name)}`);
    if (path) lines.push(`- Path: ${formatInlineCode(path)}`);
    return `\n${lines.join("\n")}\n`;
  });
}

function extractSkillField(block: string, tag: "name" | "path"): string {
  const pattern = new RegExp(`<${tag}>\\s*([\\s\\S]*?)\\s*<\\/${tag}>`, "i");
  const match = block.match(pattern);
  if (!match) return "";
  return match[1].trim();
}

function formatInlineCode(value: string): string {
  const maxBacktickRun = maxBacktickSequenceLength(value);
  const fence = "`".repeat(Math.max(1, maxBacktickRun + 1));
  const needsPadding =
    value.startsWith("`") ||
    value.endsWith("`") ||
    value.startsWith(" ") ||
    value.endsWith(" ");
  const content = needsPadding ? ` ${value} ` : value;
  return `${fence}${content}${fence}`;
}

function maxBacktickSequenceLength(value: string): number {
  let maxRun = 0;
  let currentRun = 0;
  for (const ch of value) {
    if (ch === "`") {
      currentRun += 1;
      if (currentRun > maxRun) {
        maxRun = currentRun;
      }
      continue;
    }
    currentRun = 0;
  }
  return maxRun;
}

function estimateStringBytes(text: string): number {
  return text.length * 2;
}
