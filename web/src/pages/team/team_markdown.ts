import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import { escapeTeamHtml } from "./team_text_helpers";

const TEAM_MARKDOWN_CACHE_LIMIT = 256;
const TEAM_MARKDOWN_CACHE_MAX_BYTES = 4 * 1024 * 1024;
const TEAM_MARKDOWN_CACHE_MAX_ENTRY_CHARS = 64_000;
const TEAM_AUTOLINK_MAX_INPUT_LENGTH = 120_000;
const GITHUB_PULL_URL_PATTERN =
  /^https:\/\/github\.com\/[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+\/pull\/\d+(?:[/?#][^\s<>"']*)?$/;
const URL_CANDIDATE_PATTERN = /https:\/\/[^\s<>()]+(?:\([^\s<>]*\)[^\s<>()]*)*/g;
const SKILL_BLOCK_PATTERN = /<skill>\s*([\s\S]*?)\s*<\/skill>/gi;

export const TEAM_THREAD_RICH_TEXT_BASE_CLASS =
  "acp-text min-w-0 max-w-full text-sm leading-6 [overflow-wrap:anywhere] [&_a]:font-medium [&_a]:text-sky-700 [&_a]:underline [&_a]:underline-offset-2 [&_blockquote]:my-2 [&_blockquote]:border-l-2 [&_blockquote]:border-slate-200 [&_blockquote]:pl-3 [&_blockquote]:text-slate-600 [&_code]:break-words [&_code]:rounded [&_code]:bg-slate-100 [&_code]:px-1 [&_em]:text-slate-700 [&_em]:italic [&_h1]:mb-3 [&_h1]:mt-1 [&_h1]:text-[1rem] [&_h1]:font-semibold [&_h2]:mb-2.5 [&_h2]:mt-1 [&_h2]:text-[0.96rem] [&_h2]:font-semibold [&_h3]:mb-2 [&_h3]:mt-1 [&_h3]:text-[0.9rem] [&_h3]:font-semibold [&_hr]:my-3 [&_hr]:border-slate-200 [&_li]:my-0.5 [&_li]:break-words [&_ol]:my-2 [&_ol]:max-w-full [&_p]:mb-2.5 [&_p]:break-words [&_p:last-child]:mb-0 [&_pre]:my-2.5 [&_pre]:max-w-full [&_pre]:overflow-x-auto [&_pre]:rounded-lg [&_pre]:border [&_pre]:border-slate-200 [&_pre]:bg-slate-950 [&_pre]:px-3 [&_pre]:py-2.5 [&_pre]:whitespace-pre-wrap [&_pre]:break-words [&_pre_code]:bg-transparent [&_pre_code]:px-0 [&_pre_code]:text-slate-100 [&_pre_code]:whitespace-pre-wrap [&_pre_code]:break-words [&_strong]:font-semibold [&_strong]:text-slate-900 [&_table]:my-2 [&_table]:block [&_table]:max-w-full [&_table]:overflow-x-auto [&_td]:break-words [&_td]:border-b [&_td]:border-slate-100 [&_td]:py-1.5 [&_th]:break-words [&_th]:border-b [&_th]:border-slate-200 [&_th]:py-1.5 [&_ul]:my-2 [&_ul]:max-w-full";

const markdownHtmlCache = new Map<string, string>();
const markdownHtmlCacheSize = new Map<string, number>();
let markdownCacheBytes = 0;
let markdownRenderer: MarkdownIt | null = null;

export function renderTeamMarkdownCached(text: string): string {
  const normalized = normalizeSkillBlocksForMarkdown(text);
  if (text.length > TEAM_MARKDOWN_CACHE_MAX_ENTRY_CHARS) {
    return renderTeamMarkdown(normalized);
  }
  const cached = markdownHtmlCache.get(text);
  if (cached != null) {
    refreshCacheRecency(text, cached, markdownHtmlCacheSize.get(text) ?? 0);
    return cached;
  }
  const rendered = renderTeamMarkdown(normalized);
  const estimatedBytes = estimateStringBytes(text) + estimateStringBytes(rendered);
  return cacheWithLruBudget(text, rendered, estimatedBytes);
}

function renderTeamMarkdown(input: string): string {
  const stripped = input.replace(/cite[^]+/g, "").replace(/[]/g, "");
  try {
    const prepared =
      stripped.length > TEAM_AUTOLINK_MAX_INPUT_LENGTH
        ? stripped
        : autoLinkWhitelistedUrls(stripped);
    return getMarkdownRenderer().render(prepared);
  } catch {
    return `<p>${escapeTeamHtml(stripped)}</p>`;
  }
}

function getMarkdownRenderer(): MarkdownIt {
  if (markdownRenderer) return markdownRenderer;
  const renderer = new MarkdownIt({
    html: false,
    linkify: false,
    typographer: true,
    highlight: (code: string, lang: string) => {
      const language = lang ? lang.trim().split(/\s+/)[0] : "";
      if (language && hljs.getLanguage(language)) {
        try {
          return `<pre class="hljs"><code>${hljs.highlight(code, { language }).value}</code></pre>`;
        } catch {
          // fallback to escaped text
        }
      }
      return `<pre class="hljs"><code>${escapeTeamHtml(code)}</code></pre>`;
    },
  });
  renderer.validateLink = (href: string) => sanitizeHref(href) != null;
  renderer.normalizeLink = (href: string) => sanitizeHref(href) ?? "";
  const defaultLinkOpen =
    renderer.renderer.rules.link_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.link_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrSet("target", "_blank");
    tokens[idx].attrSet("rel", "noopener noreferrer");
    return defaultLinkOpen(tokens, idx, options, env, self);
  };
  markdownRenderer = renderer;
  return renderer;
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

function decodeHtmlEntities(input: string): string {
  const named: Record<string, string> = {
    amp: "&",
    colon: ":",
    tab: "\t",
    newline: "\n",
  };
  let out = input;
  for (let i = 0; i < 2; i += 1) {
    out = out.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (_match, body) => {
      const key = String(body).toLowerCase();
      if (key in named) return named[key];
      if (key.startsWith("#x")) {
        const code = Number.parseInt(key.slice(2), 16);
        if (Number.isNaN(code)) return "";
        return String.fromCharCode(code);
      }
      if (key.startsWith("#")) {
        const code = Number.parseInt(key.slice(1), 10);
        if (Number.isNaN(code)) return "";
        return String.fromCharCode(code);
      }
      return "";
    });
  }
  return out;
}

function sanitizeHref(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const decoded = decodeHtmlEntities(trimmed);
  // eslint-disable-next-line no-control-regex
  const normalized = decoded.replace(/[\u0000-\u001F\u007F\s]+/g, "").toLowerCase();
  if (
    normalized.startsWith("http://") ||
    normalized.startsWith("https://") ||
    normalized.startsWith("mailto:") ||
    normalized.startsWith("tel:")
  ) {
    return trimmed;
  }
  if (
    normalized.startsWith("/") ||
    normalized.startsWith("./") ||
    normalized.startsWith("../") ||
    normalized.startsWith("#")
  ) {
    return trimmed;
  }
  return null;
}

function isWhitelistedAutoLinkUrl(url: string): boolean {
  return GITHUB_PULL_URL_PATTERN.test(url);
}

function splitUrlAndTrailingPunctuation(raw: string): { url: string; trailing: string } {
  let end = raw.length;
  while (end > 0) {
    const ch = raw.charAt(end - 1);
    if (ch === "." || ch === "," || ch === ";" || ch === ":" || ch === "!" || ch === "?") {
      end -= 1;
      continue;
    }
    break;
  }
  return {
    url: raw.slice(0, end),
    trailing: raw.slice(end),
  };
}

function isLikelyExistingLinkContext(segment: string, index: number): boolean {
  const prev = index > 0 ? segment.charAt(index - 1) : "";
  if (prev === "(" || prev === "<") return true;
  if (index >= 2 && segment.slice(index - 2, index) === "](") return true;
  return false;
}

function splitInlineCodeSegments(
  line: string
): { text: string; isCode: boolean }[] {
  const segments: { text: string; isCode: boolean }[] = [];
  const length = line.length;
  let i = 0;
  let lastIndex = 0;
  let inCode = false;
  let delimiterLength = 0;
  let codeStart = 0;

  while (i < length) {
    const ch = line.charAt(i);
    if (!inCode) {
      if (ch !== "`") {
        i += 1;
        continue;
      }
      let j = i;
      while (j < length && line.charAt(j) === "`") {
        j += 1;
      }
      if (i > lastIndex) {
        segments.push({
          text: line.slice(lastIndex, i),
          isCode: false,
        });
      }
      inCode = true;
      delimiterLength = j - i;
      codeStart = i;
      i = j;
      lastIndex = codeStart;
      continue;
    }
    if (ch !== "`") {
      i += 1;
      continue;
    }
    let j = i;
    while (j < length && line.charAt(j) === "`") {
      j += 1;
    }
    const runLength = j - i;
    if (runLength === delimiterLength) {
      segments.push({
        text: line.slice(codeStart, j),
        isCode: true,
      });
      inCode = false;
      delimiterLength = 0;
      i = j;
      lastIndex = i;
      continue;
    }
    i = j;
  }

  if (!inCode && lastIndex < length) {
    segments.push({
      text: line.slice(lastIndex),
      isCode: false,
    });
  } else if (inCode) {
    segments.push({
      text: line.slice(codeStart),
      isCode: false,
    });
  }
  return segments;
}

function autoLinkWhitelistedUrlsInLine(line: string): string {
  const segments = splitInlineCodeSegments(line);
  return segments
    .map((segment) => {
      if (segment.isCode) return segment.text;
      return segment.text.replace(URL_CANDIDATE_PATTERN, (matched, offset, source) => {
        const start = Number(offset);
        const sourceSegment = String(source);
        if (isLikelyExistingLinkContext(sourceSegment, start)) {
          return matched;
        }
        const { url, trailing } = splitUrlAndTrailingPunctuation(matched);
        if (!url || !isWhitelistedAutoLinkUrl(url)) {
          return matched;
        }
        return `[${url}](${url})${trailing}`;
      });
    })
    .join("");
}

function autoLinkWhitelistedUrls(input: string): string {
  const lines = input.split("\n");
  const output: string[] = [];
  let inFence = false;
  let fenceToken = "";
  let fenceLength = 0;
  for (const line of lines) {
    const trimmed = line.trimStart();
    const fenceMatch = trimmed.match(/^(```+|~~~+)/);
    if (fenceMatch) {
      const nextFence = fenceMatch[1];
      if (!inFence) {
        inFence = true;
        fenceToken = nextFence.charAt(0);
        fenceLength = nextFence.length;
      } else if (
        nextFence.charAt(0) === fenceToken &&
        nextFence.length >= fenceLength
      ) {
        inFence = false;
        fenceToken = "";
        fenceLength = 0;
      }
      output.push(line);
      continue;
    }
    output.push(inFence ? line : autoLinkWhitelistedUrlsInLine(line));
  }
  return output.join("\n");
}
