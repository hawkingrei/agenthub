import React from "react";
import {
  renderTeamMarkdownCached,
  TEAM_THREAD_RICH_TEXT_BASE_CLASS,
} from "./team_markdown";

/**
 * Must return fully sanitized HTML that is safe to pass into
 * dangerouslySetInnerHTML.
 */
export type SanitizedHtmlRenderer = (text: string) => string;

export function TeamThreadRichText({
  text,
  className,
  renderSanitizedHtml = renderTeamMarkdownCached,
}: {
  text: string;
  className?: string;
  renderSanitizedHtml?: SanitizedHtmlRenderer;
}) {
  const html = renderSanitizedHtml(text);
  return (
    <div
      className={`${TEAM_THREAD_RICH_TEXT_BASE_CLASS} ${className ?? ""}`.trim()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
