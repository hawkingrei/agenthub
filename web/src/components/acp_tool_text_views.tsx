import React from "react";
import {
  ACP_DIFF_PRE_CLASS,
  ACP_PAYLOAD_MARKDOWN_CLASS,
  ACP_SEGMENTED_NOTE_WARNING_CLASS,
  ACP_TERMINAL_PRE_CLASS,
} from "../ui/tailwind_classes";
import {
  SegmentedMoreFooter,
  useProgressiveTailWindow,
} from "./acp_progressive_views";
import { ThreadRichText } from "./thread_rich_text";

export const TOOL_TEXT_INITIAL_LINES = 36;
export const TOOL_TEXT_LINE_CHUNK = 120;
export const ACP_SEGMENTED_BLOCK_CLASS = "acp-segmented-block grid gap-1.5";

const TOOL_TEXT_MARKDOWN_FALLBACK_LINES = 260;
const TOOL_TEXT_MARKDOWN_FALLBACK_LENGTH = 16000;
const TOOL_TEXT_CLASS_ATTRIBUTE_DISPLAY_LIMIT = 72;
const ACP_PAYLOAD_TEXT_BASE_CLASS =
  "acp-content acp-payload-text m-0 whitespace-pre-wrap font-mono text-[11px] leading-[1.5] text-slate-800";
const ACP_PAYLOAD_TEXT_ASCII_CLASS =
  `${ACP_PAYLOAD_TEXT_BASE_CLASS} acp-payload-ascii overflow-x-auto whitespace-pre`;
const ACP_CONTENT_TEXT_BASE_CLASS =
  `${ACP_TERMINAL_PRE_CLASS} acp-content acp-payload-text`;
const ACP_CONTENT_TEXT_ASCII_CLASS =
  `${ACP_CONTENT_TEXT_BASE_CLASS} acp-payload-ascii whitespace-pre`;
const ACP_CONTENT_MARKDOWN_CLASS =
  "acp-content-markdown rounded-lg border border-slate-200 bg-white/92 px-3 py-2.5 text-[13px] leading-6 text-slate-900 shadow-none [&_.hljs]:bg-transparent [&_a]:text-sky-700 [&_blockquote]:border-l-2 [&_blockquote]:border-slate-200 [&_blockquote]:pl-3 [&_blockquote]:text-slate-600 [&_code]:rounded [&_code]:bg-slate-100 [&_code]:px-1 [&_code]:text-slate-900 [&_li]:marker:text-slate-400 [&_p]:mb-2.5 [&_p:last-child]:mb-0 [&_pre]:my-2.5 [&_pre]:border-0 [&_pre]:bg-transparent [&_pre]:p-0 [&_pre]:text-inherit [&_strong]:text-slate-900";

type DiffLineKind = "meta" | "hunk" | "add" | "remove" | "context";

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

export function createPayloadTextRenderers(): {
  renderText: (text: string) => React.ReactNode;
  renderPlainText: (text: string) => React.ReactNode;
} {
  return {
    renderText: (text: string) => (
      <ToolTextContent text={text} markdownClassName={ACP_PAYLOAD_MARKDOWN_CLASS} />
    ),
    renderPlainText: (text: string) => (
      <ToolPlainTextView text={text} asciiLike={shouldPreserveAsciiText(text)} />
    ),
  };
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
  const displayText = React.useMemo(
    () => compressLongHtmlClassAttributesForDisplay(visibleText),
    [visibleText]
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
      <pre className={className} title={displayText !== visibleText ? visibleText : undefined}>
        {displayText}
      </pre>
      {hasMore && (
        <SegmentedMoreFooter remaining={remaining} unitLabel="lines" onShowMore={showMore} />
      )}
    </div>
  );
}

export function compressLongHtmlClassAttributesForDisplay(text: string): string {
  if (!text.includes("<") || !text.includes('class="')) {
    return text;
  }
  return text.replace(/(<[A-Za-z][^>\n]*?)class="([^"]+)"/g, (_match, prefix: string, classValue: string) => {
    if (classValue.length <= TOOL_TEXT_CLASS_ATTRIBUTE_DISPLAY_LIMIT) {
      return `${prefix}class="${classValue}"`;
    }
    const truncated = `${classValue.slice(0, TOOL_TEXT_CLASS_ATTRIBUTE_DISPLAY_LIMIT - 1)}…`;
    return `${prefix}class="${truncated}"`;
  });
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
