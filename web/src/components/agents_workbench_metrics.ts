export type AgentsWorkbenchCacheStats = {
  markdownHits: number;
  markdownMisses: number;
  ansiHits: number;
  ansiMisses: number;
  payloadParses: number;
  payloadParseFailures: number;
};

export type AgentsWorkbenchConversationStats = {
  totalItems: number;
  sourceItems: number;
  renderedItems: number;
  pendingItems: number;
  virtualized: boolean;
  stickToBottom: boolean;
  averageHeight: number;
};

export function buildAcpRuntimeMetrics(params: {
  rawEventCount: number;
  toolCallCount: number;
  messageCount: number;
  conversation: AgentsWorkbenchConversationStats;
  cacheStats: AgentsWorkbenchCacheStats;
}) {
  const {
    rawEventCount,
    toolCallCount,
    messageCount,
    conversation,
    cacheStats,
  } = params;
  return {
    totalConversationItems: conversation.totalItems,
    sourceConversationItems: conversation.sourceItems,
    renderedConversationItems: conversation.renderedItems,
    pendingConversationItems: conversation.pendingItems,
    virtualizedConversation: conversation.virtualized,
    stickToBottom: conversation.stickToBottom,
    averageConversationHeight: Math.round(conversation.averageHeight),
    rawEventCount,
    toolCallCount,
    messageCount,
    markdownCacheHits: cacheStats.markdownHits,
    markdownCacheMisses: cacheStats.markdownMisses,
    ansiCacheHits: cacheStats.ansiHits,
    ansiCacheMisses: cacheStats.ansiMisses,
    payloadParses: cacheStats.payloadParses,
    payloadParseFailures: cacheStats.payloadParseFailures,
  };
}
