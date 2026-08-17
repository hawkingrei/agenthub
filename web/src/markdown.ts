import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import { escapeHtml } from "./html_escape";
import { createConfiguredMarkdownRenderer } from "./markdown_renderer";

const GITHUB_PULL_URL_PATTERN =
  /^https:\/\/github\.com\/[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+\/pull\/\d+(?:[/?#][^\s<>"']*)?$/;
const URL_CANDIDATE_PATTERN = /https:\/\/[^\s<>()]+(?:\([^\s<>]*\)[^\s<>()]*)*/g;
const AUTOLINK_MAX_INPUT_LENGTH = 120_000;

function decodeHtmlEntities(input: string): string {
  const named: Record<string, string> = {
    amp: "&",
    colon: ":",
    tab: "\t",
    newline: "\n",
  };
  let out = input;
  for (let i = 0; i < 2; i += 1) {
    out = out.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (_m, body) => {
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

export function renderMarkdown(input: string): string {
  const stripped = input.replace(/cite[^]+/g, "").replace(/[]/g, "");
  try {
    const prepared =
      stripped.length > AUTOLINK_MAX_INPUT_LENGTH
        ? stripped
        : autoLinkWhitelistedUrls(stripped);
    return getMarkdownRenderer().render(prepared);
  } catch {
    return `<p>${escapeHtml(stripped)}</p>`;
  }
}

let markdownRenderer: InstanceType<typeof MarkdownIt> | null = null;

function getMarkdownRenderer(): InstanceType<typeof MarkdownIt> {
  if (markdownRenderer) return markdownRenderer;
  const renderer = createConfiguredMarkdownRenderer({
    highlight: (code: string, lang: string) => {
      const language = lang ? lang.trim().split(/\s+/)[0] : "";
      if (language && hljs.getLanguage(language)) {
        try {
          return `<pre class="hljs"><code>${hljs.highlight(code, { language }).value}</code></pre>`;
        } catch {
          // fallback to escaped text
        }
      }
      return `<pre class="hljs"><code>${escapeHtml(code)}</code></pre>`;
    },
    sanitizeHref,
  });
  markdownRenderer = renderer;
  return renderer;
}
