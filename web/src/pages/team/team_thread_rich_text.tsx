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
  onMentionClick,
}: {
  text: string;
  className?: string;
  renderSanitizedHtml?: SanitizedHtmlRenderer;
  onMentionClick?: (actorId: string) => void;
}) {
  const html = renderSanitizedHtml(text);
  return (
    <div
      className={`${TEAM_THREAD_RICH_TEXT_BASE_CLASS} ${className ?? ""}`.trim()}
      onClick={(event) => {
        if (!onMentionClick) {
          return;
        }
        const mention = (event.target as HTMLElement | null)?.closest(
          "[data-team-agent-mention-id]"
        );
        const actorId = mention?.getAttribute("data-team-agent-mention-id")?.trim();
        if (actorId) {
          onMentionClick(actorId);
        }
      }}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
