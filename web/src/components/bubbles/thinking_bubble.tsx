import React from "react";
import {
  formatConversationPreview,
  unescapeLineBreaks,
} from "../../conversation";
import {
  ACP_BUBBLE_THINKING_CLASS,
  CONVERSATION_MESSAGE_STACK_ROW_CLASS,
} from "../../ui/tailwind_classes";
import { ThreadRichText } from "../thread_rich_text";

export type ThinkingBubbleProps = {
  text: string;
  live?: boolean;
  summaryPrefix?: string;
  grouped?: boolean;
};

export const ThinkingBubble = React.memo(function ThinkingBubble({
  text,
  live = false,
  summaryPrefix,
  grouped = false,
}: ThinkingBubbleProps) {
  const summary = deriveThinkingSummary(text, {
    live,
    summaryPrefix,
  });
  const entryClassName = grouped
    ? ""
    : `acp-row ${CONVERSATION_MESSAGE_STACK_ROW_CLASS} items-start`;

  return (
    <div className={entryClassName}>
      <div className={ACP_BUBBLE_THINKING_CLASS}>
        <details className="acp-thought-fold acp-thinking-fold">
          <summary className="cursor-pointer text-sm font-semibold text-notion-text opacity-80">
            {summary}
          </summary>
          <div className="acp-text mt-2 text-[14px] leading-6 text-notion-text opacity-90">
            <ThreadRichText text={text} />
          </div>
        </details>
      </div>
    </div>
  );
});

function deriveThinkingSummary(
  text: string,
  {
    live,
    summaryPrefix,
  }: {
    live?: boolean;
    summaryPrefix?: string;
  } = {}
): string {
  const normalizedText = unescapeLineBreaks(text);
  const firstLine = normalizedText
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line.length > 0);
  const preview = firstLine
    ? formatConversationPreview(normalizeThinkingSummaryLine(firstLine), 96)
    : "THINKING";
  const prefix = summaryPrefix ?? "THINKING";
  const base = preview === prefix ? prefix : `${prefix} · ${preview}`;
  return live ? `${base} (live)` : base;
}

function normalizeThinkingSummaryLine(line: string): string {
  const normalized = line
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/[`*_~>#]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  return normalized || line.trim();
}
