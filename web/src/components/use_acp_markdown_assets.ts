import React from "react";
import { preloadThreadMarkdownAssets } from "./thread_rich_text";

export function useAcpMarkdownRenderVersion(): number {
  const [markdownRenderVersion, setMarkdownRenderVersion] = React.useState(0);

  React.useEffect(() => {
    let cancelled = false;
    void preloadThreadMarkdownAssets()
      .then(() => {
        if (!cancelled) {
          setMarkdownRenderVersion(1);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  return markdownRenderVersion;
}
