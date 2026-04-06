import React from "react";
import {
  renderTeamMarkdownCached,
  TEAM_THREAD_RICH_TEXT_BASE_CLASS,
} from "./team_markdown";

export function TeamThreadRichText({
  text,
  className,
  renderHtml = renderTeamMarkdownCached,
}: {
  text: string;
  className?: string;
  renderHtml?: (text: string) => string;
}) {
  const html = renderHtml(text);
  return (
    <div
      className={`${TEAM_THREAD_RICH_TEXT_BASE_CLASS} ${className ?? ""}`.trim()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
