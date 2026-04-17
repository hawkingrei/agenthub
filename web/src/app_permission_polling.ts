import type { AcpPermissionRecord } from "./api";

export const GLOBAL_PERMISSION_POLL_INTERVAL_MS = 5000;
export const GLOBAL_PERMISSION_POLL_INTERVAL_COLLAPSED_MS = 10000;
export const GLOBAL_PERMISSION_POLL_MAX_CONCURRENCY = 4;
export const GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS = 3000;
export const GLOBAL_PERMISSION_POLL_IDLE_DELAY_MS = 10000;

export function parsePermissionPollAgentIds(key: string): string[] {
  return key
    .split(",")
    .map((agentId) => agentId.trim())
    .filter(Boolean);
}

export function buildGlobalPermissionPollAgentIds(
  allAgentIds: string[],
  activeAgent: string | null
): string[] {
  if (!activeAgent) return allAgentIds;
  return allAgentIds.filter((agentId) => agentId !== activeAgent);
}

export function resolveGlobalPermissionPollIntervalMs(
  agentsCollapsed: boolean
): number {
  return agentsCollapsed
    ? GLOBAL_PERMISSION_POLL_INTERVAL_COLLAPSED_MS
    : GLOBAL_PERMISSION_POLL_INTERVAL_MS;
}

export function chunkPermissionPollAgentIds(
  agentIds: string[],
  maxConcurrency: number
): string[][] {
  const limit = Math.max(1, Math.floor(maxConcurrency));
  const chunks: string[][] = [];
  for (let i = 0; i < agentIds.length; i += limit) {
    chunks.push(agentIds.slice(i, i + limit));
  }
  return chunks;
}

export function buildPendingPermissionCountMap(
  entries: ReadonlyArray<readonly [string, number]>
): Record<string, number> {
  const nextCounts: Record<string, number> = {};
  for (const [agentId, count] of entries) {
    if (count > 0) {
      nextCounts[agentId] = count;
    }
  }
  return nextCounts;
}

export function mergePendingPermissionCountMap(
  prev: Record<string, number>,
  allAgentIds: string[],
  updates: ReadonlyArray<readonly [string, number | null]>
): Record<string, number> {
  const nextCounts: Record<string, number> = {};
  const allAgentSet = new Set(allAgentIds);
  const updatedAgentSet = new Set(updates.map(([agentId]) => agentId));

  for (const [agentId, count] of Object.entries(prev)) {
    if (!allAgentSet.has(agentId)) continue;
    if (!updatedAgentSet.has(agentId) && count > 0) {
      nextCounts[agentId] = count;
    }
  }

  for (const [agentId, count] of updates) {
    if (count == null) {
      const prevCount = prev[agentId];
      if (typeof prevCount === "number" && prevCount > 0) {
        nextCounts[agentId] = prevCount;
      }
      continue;
    }
    if (count > 0) {
      nextCounts[agentId] = count;
    } else {
      delete nextCounts[agentId];
    }
  }
  return nextCounts;
}

export function filterPermissionsForAgent(
  items: AcpPermissionRecord[],
  agentId: string | null
): AcpPermissionRecord[] {
  if (!agentId) return [];
  return items.filter(
    (item) => item.agent_id === agentId && shouldDisplayPermissionRecord(item)
  );
}

export function shouldDisplayPermissionRecord(item: AcpPermissionRecord): boolean {
  const status = item.status.trim().toLowerCase();
  if (status === "timeout") {
    return false;
  }
  if (status !== "responded") {
    return true;
  }
  const selectedOptionId =
    typeof item.selected_option_id === "string" ? item.selected_option_id.trim() : "";
  return selectedOptionId.length === 0;
}

function defaultScheduleTimeout(
  callback: () => void,
  delayMs: number
): number {
  return globalThis.setTimeout(callback, delayMs) as unknown as number;
}

function defaultClearTimeout(timerId: number): void {
  globalThis.clearTimeout(
    timerId as unknown as ReturnType<typeof globalThis.setTimeout>
  );
}

export function schedulePermissionPollLoop(
  delay: number,
  pollState: { timer: number | null },
  pollOnce: () => Promise<number>,
  isCancelled: () => boolean,
  scheduleTimeout: (callback: () => void, delayMs: number) => number = defaultScheduleTimeout,
  clearTimeoutFn: (timerId: number) => void = defaultClearTimeout
): void {
  if (isCancelled()) return;
  if (pollState.timer != null) {
    clearTimeoutFn(pollState.timer);
    pollState.timer = null;
  }
  pollState.timer = scheduleTimeout(async () => {
    if (isCancelled()) {
      pollState.timer = null;
      return;
    }
    const pendingCount = await pollOnce().catch(() => 0);
    if (isCancelled()) {
      pollState.timer = null;
      return;
    }
    const nextDelay =
      pendingCount > 0
        ? GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS
        : GLOBAL_PERMISSION_POLL_IDLE_DELAY_MS;
    schedulePermissionPollLoop(
      nextDelay,
      pollState,
      pollOnce,
      isCancelled,
      scheduleTimeout,
      clearTimeoutFn
    );
  }, delay);
}
