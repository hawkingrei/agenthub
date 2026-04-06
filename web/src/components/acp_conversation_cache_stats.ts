import { getToolContentCacheStats } from "./acp_tool_content";
import { getThreadMarkdownCacheStats } from "./thread_rich_text";

export type AcpConversationCacheStats = {
  markdownHits: number;
  markdownMisses: number;
  ansiHits: number;
  ansiMisses: number;
  payloadParses: number;
  payloadParseFailures: number;
};

export function getAcpConversationCacheStats(): AcpConversationCacheStats {
  const markdownStats = getThreadMarkdownCacheStats();
  const toolContentStats = getToolContentCacheStats();
  return {
    markdownHits: markdownStats.markdownHits,
    markdownMisses: markdownStats.markdownMisses,
    ansiHits: toolContentStats.ansiHits,
    ansiMisses: toolContentStats.ansiMisses,
    payloadParses: toolContentStats.payloadParses,
    payloadParseFailures: toolContentStats.payloadParseFailures,
  };
}
