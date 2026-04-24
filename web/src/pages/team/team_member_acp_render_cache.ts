import type { AgentEvent } from "../../api";

const MAX_MEMBER_ACP_RENDER_CACHE_ENTRIES = 24;

const memberAcpRenderCache = new Map<string, AgentEvent[]>();

function normalizeCacheKey(agentId: string, sessionId: string | null | undefined): string {
  const normalizedAgentId = agentId.trim();
  const normalizedSessionId = sessionId?.trim() ?? "";
  if (!normalizedAgentId || !normalizedSessionId) {
    return "";
  }
  return `${normalizedAgentId}:${normalizedSessionId}`;
}

export function loadTeamMemberAcpRenderCache(
  agentId: string,
  sessionId: string | null | undefined
): AgentEvent[] {
  const key = normalizeCacheKey(agentId, sessionId);
  if (!key) {
    return [];
  }
  const cached = memberAcpRenderCache.get(key);
  if (!cached) {
    return [];
  }
  memberAcpRenderCache.delete(key);
  memberAcpRenderCache.set(key, cached);
  return cached;
}

export function saveTeamMemberAcpRenderCache(
  agentId: string,
  sessionId: string | null | undefined,
  events: AgentEvent[]
): void {
  const key = normalizeCacheKey(agentId, sessionId);
  if (!key || events.length === 0) {
    return;
  }
  memberAcpRenderCache.delete(key);
  memberAcpRenderCache.set(key, events);
  while (memberAcpRenderCache.size > MAX_MEMBER_ACP_RENDER_CACHE_ENTRIES) {
    const oldestKey = memberAcpRenderCache.keys().next().value;
    if (!oldestKey) {
      break;
    }
    memberAcpRenderCache.delete(oldestKey);
  }
}

export function clearTeamMemberAcpRenderCache(): void {
  memberAcpRenderCache.clear();
}
