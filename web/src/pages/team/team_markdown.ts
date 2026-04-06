import { renderMarkdown } from "../../markdown";

const TEAM_MARKDOWN_CACHE_LIMIT = 256;
const TEAM_MARKDOWN_CACHE_MAX_BYTES = 4 * 1024 * 1024;
const TEAM_MARKDOWN_CACHE_MAX_ENTRY_CHARS = 64_000;
const SKILL_BLOCK_PATTERN = /<skill>\s*([\s\S]*?)\s*<\/skill>/gi;

export const TEAM_THREAD_RICH_TEXT_BASE_CLASS =
  "acp-text min-w-0 max-w-full text-sm leading-6 [overflow-wrap:anywhere] [&_code]:break-words [&_li]:break-words [&_ol]:max-w-full [&_p]:break-words [&_pre]:max-w-full [&_pre]:whitespace-pre-wrap [&_pre]:break-words [&_pre_code]:whitespace-pre-wrap [&_pre_code]:break-words [&_table]:block [&_table]:max-w-full [&_table]:overflow-x-auto [&_td]:break-words [&_th]:break-words [&_ul]:max-w-full";

const markdownHtmlCache = new Map<string, string>();
const markdownHtmlCacheSize = new Map<string, number>();
let markdownCacheBytes = 0;

export function renderTeamMarkdownCached(text: string): string {
  const normalized = normalizeSkillBlocksForMarkdown(text);
  if (text.length > TEAM_MARKDOWN_CACHE_MAX_ENTRY_CHARS) {
    return renderMarkdown(normalized);
  }
  const cached = markdownHtmlCache.get(text);
  if (cached != null) {
    refreshCacheRecency(text, cached, markdownHtmlCacheSize.get(text) ?? 0);
    return cached;
  }
  const rendered = renderMarkdown(normalized);
  const estimatedBytes = estimateStringBytes(text) + estimateStringBytes(rendered);
  return cacheWithLruBudget(text, rendered, estimatedBytes);
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

function cacheWithLruBudget(key: string, value: string, size: number): string {
  if (markdownHtmlCache.has(key)) {
    const previousSize = markdownHtmlCacheSize.get(key) ?? 0;
    markdownCacheBytes = Math.max(0, markdownCacheBytes - previousSize);
    markdownHtmlCacheSize.delete(key);
    markdownHtmlCache.delete(key);
  }
  markdownHtmlCache.set(key, value);
  markdownHtmlCacheSize.set(key, size);
  markdownCacheBytes += size;
  while (
    markdownHtmlCache.size > TEAM_MARKDOWN_CACHE_LIMIT ||
    markdownCacheBytes > TEAM_MARKDOWN_CACHE_MAX_BYTES
  ) {
    const oldestKey = markdownHtmlCache.keys().next().value;
    if (oldestKey === undefined) break;
    const oldestSize = markdownHtmlCacheSize.get(oldestKey) ?? 0;
    markdownCacheBytes = Math.max(0, markdownCacheBytes - oldestSize);
    markdownHtmlCacheSize.delete(oldestKey);
    markdownHtmlCache.delete(oldestKey);
  }
  return value;
}

function refreshCacheRecency(key: string, value: string, size: number): void {
  markdownHtmlCache.delete(key);
  markdownHtmlCache.set(key, value);
  markdownHtmlCacheSize.delete(key);
  markdownHtmlCacheSize.set(key, size);
}

function estimateStringBytes(text: string): number {
  return text.length * 2;
}
