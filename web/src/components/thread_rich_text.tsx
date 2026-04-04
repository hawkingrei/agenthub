import React from "react";
import { renderMarkdown } from "../markdown";
import "highlight.js/styles/github-dark.css";

const MARKDOWN_CACHE_LIMIT = 512;
const MARKDOWN_CACHE_MAX_BYTES = 8 * 1024 * 1024;
const MARKDOWN_CACHE_MAX_ENTRY_CHARS = 120_000;
const SKILL_BLOCK_PATTERN = /<skill>\s*([\s\S]*?)\s*<\/skill>/gi;

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
  const estimatedBytes =
    estimateStringBytes(text) + estimateStringBytes(rendered);
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

export function ThreadRichText({
  text,
  className,
  renderHtml = renderThreadMarkdownCached,
}: {
  text: string;
  className?: string;
  renderHtml?: (text: string) => string;
}) {
  const html = renderHtml(text);
  return (
    <div
      className={`acp-text text-sm leading-6 ${className ?? ""}`.trim()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
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

function refreshCacheRecency<K, V>(
  cache: Map<K, V>,
  sizes: Map<K, number>,
  key: K
): void {
  const value = cache.get(key);
  if (value == null) {
    return;
  }
  const size = sizes.get(key);
  cache.delete(key);
  cache.set(key, value);
  if (size != null) {
    sizes.delete(key);
    sizes.set(key, size);
  }
}

function estimateStringBytes(text: string): number {
  return text.length * 2;
}
