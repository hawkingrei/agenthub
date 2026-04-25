import "highlight.js/styles/github-dark.css";
import {
  getThreadMarkdownCacheStats,
  preloadThreadMarkdownAssets,
  renderThreadMarkdownCached,
  resetThreadMarkdownCache,
  THREAD_RICH_TEXT_BASE_CLASS,
  type ThreadMarkdownCacheStats,
} from "../thread_markdown";

export type { ThreadMarkdownCacheStats };

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
      className={`${THREAD_RICH_TEXT_BASE_CLASS} ${className ?? ""}`.trim()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

export {
  getThreadMarkdownCacheStats,
  preloadThreadMarkdownAssets,
  renderThreadMarkdownCached,
  resetThreadMarkdownCache,
};
