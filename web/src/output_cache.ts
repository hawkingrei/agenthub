import { AgentEvent } from "./api";
import { compareEventOrder } from "./seq_order";

export type OutputLine = AgentEvent;

const compareOutputLines = (a: OutputLine, b: OutputLine): number => {
  const base = compareEventOrder(a, b);
  if (base !== 0) return base;
  if (a.stream !== b.stream) return a.stream < b.stream ? -1 : 1;
  if (a.message !== b.message) return a.message < b.message ? -1 : 1;
  return 0;
};

const isSameOutputLine = (a: OutputLine, b: OutputLine): boolean =>
  a.event_id === b.event_id &&
  a.ts === b.ts &&
  a.seq === b.seq &&
  a.stream === b.stream &&
  a.message === b.message &&
  a.session_id === b.session_id &&
  a.agent_id === b.agent_id;

const findOutputLineIndexByEventId = (
  lines: OutputLine[],
  eventId: number
): number => {
  for (let idx = 0; idx < lines.length; idx++) {
    if (lines[idx].event_id === eventId) {
      return idx;
    }
  }
  return -1;
};

const locateSortedInsertIndex = (
  lines: OutputLine[],
  candidate: OutputLine
): number => {
  let low = 0;
  let high = lines.length;
  while (low < high) {
    const mid = Math.floor((low + high) / 2);
    if (compareOutputLines(lines[mid], candidate) <= 0) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  return low;
};

export function mergeOutputs(
  existing: OutputLine[],
  incoming: OutputLine[]
): OutputLine[] {
  const byEventId = new Map<number, OutputLine>();
  for (const line of existing) {
    byEventId.set(line.event_id, line);
  }
  for (const line of incoming) {
    byEventId.set(line.event_id, line);
  }
  return Array.from(byEventId.values()).sort(compareOutputLines);
}

export function mergeOutputsWithLimit(
  existing: OutputLine[],
  incoming: OutputLine[],
  maxRetainedEvents: number
): OutputLine[] {
  const merged = mergeOutputs(existing, incoming);
  if (maxRetainedEvents <= 0) {
    return merged;
  }
  if (merged.length <= maxRetainedEvents) {
    return merged;
  }
  return merged.slice(merged.length - maxRetainedEvents);
}

export function appendOutputLine(
  existing: OutputLine[],
  line: OutputLine
): OutputLine[] {
  if (existing.length === 0) return [line];
  const existingIndex = findOutputLineIndexByEventId(existing, line.event_id);

  if (existingIndex >= 0) {
    const previous = existing[existingIndex];
    if (isSameOutputLine(previous, line)) return existing;
    const withoutExisting = [
      ...existing.slice(0, existingIndex),
      ...existing.slice(existingIndex + 1),
    ];
    const insertIndex = locateSortedInsertIndex(withoutExisting, line);
    const next = [...withoutExisting];
    next.splice(insertIndex, 0, line);
    return next;
  }

  const insertIndex = locateSortedInsertIndex(existing, line);
  if (insertIndex === existing.length) {
    return [...existing, line];
  }
  if (insertIndex === 0) {
    return [line, ...existing];
  }
  const next = [...existing];
  next.splice(insertIndex, 0, line);
  return next;
}

export function isSameOutputList(a: OutputLine[], b: OutputLine[]): boolean {
  if (a.length !== b.length) return false;
  if (a.length === 0) return true;
  const aFirst = a[0];
  const bFirst = b[0];
  const aLast = a[a.length - 1];
  const bLast = b[b.length - 1];
  return (
    (aFirst.event_id ?? null) === (bFirst.event_id ?? null) &&
    (aLast.event_id ?? null) === (bLast.event_id ?? null) &&
    aLast.message === bLast.message &&
    aLast.stream === bLast.stream
  );
}

export function mergeOutputsPreserveHistory(
  existing: OutputLine[],
  cached: OutputLine[],
  sameKey: boolean
): OutputLine[] {
  if (!sameKey) return cached;
  if (existing.length === 0) return cached;
  if (cached.length === 0) return existing;
  return mergeOutputs(existing, cached);
}

export function buildAcpCacheSlice(
  existing: OutputLine[],
  ordered: OutputLine[],
  maxCachedEvents: number
): OutputLine[] {
  const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
  const merged = mergeOutputs(existing, acpOrdered);
  if (merged.length <= maxCachedEvents) return merged;
  return merged.slice(merged.length - maxCachedEvents);
}

export function replaceAcpCacheSlice(
  ordered: OutputLine[],
  maxCachedEvents: number
): OutputLine[] {
  const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
  if (maxCachedEvents > 0 && acpOrdered.length > maxCachedEvents) {
    return acpOrdered.slice(acpOrdered.length - maxCachedEvents);
  }
  return acpOrdered;
}

export function buildOutputCacheSlice(
  existing: OutputLine[],
  ordered: OutputLine[],
  maxCachedEvents: number
): OutputLine[] {
  const merged = mergeOutputs(existing, ordered);
  if (maxCachedEvents <= 0) return merged;
  if (merged.length <= maxCachedEvents) return merged;
  return merged.slice(merged.length - maxCachedEvents);
}

export function limitOutputCacheSessions(
  cache: Record<string, OutputLine[]>,
  maxCachedSessions: number
): Record<string, OutputLine[]> {
  if (maxCachedSessions <= 0) return {};
  const entries = Object.entries(cache).filter(([, lines]) => lines.length > 0);
  if (entries.length <= maxCachedSessions) return cache;
  const limited = entries
    .sort((a, b) => getOutputListRecency(b[1]) - getOutputListRecency(a[1]))
    .slice(0, maxCachedSessions);
  return Object.fromEntries(limited);
}

export type CachedOutputSelection = {
  outputs: OutputLine[] | null;
  acpOutputs: OutputLine[] | null;
  source: "session" | "latest" | "none";
};

export function selectCachedOutputs(
  outputCache: Record<string, OutputLine[]>,
  acpOutputCache: Record<string, OutputLine[]>,
  key: string,
  latestKey: string
): CachedOutputSelection {
  const sessionOutputs = outputCache[key] ?? [];
  const sessionAcp = acpOutputCache[key] ?? [];
  if (sessionOutputs.length > 0 || sessionAcp.length > 0) {
    return {
      outputs: sessionOutputs.length > 0 ? sessionOutputs : null,
      acpOutputs: sessionAcp.length > 0 ? sessionAcp : null,
      source: "session",
    };
  }
  const latestOutputs = outputCache[latestKey] ?? [];
  const latestAcp = acpOutputCache[latestKey] ?? [];
  if (latestOutputs.length > 0 || latestAcp.length > 0) {
    return {
      outputs: latestOutputs.length > 0 ? latestOutputs : null,
      acpOutputs: latestAcp.length > 0 ? latestAcp : null,
      source: "latest",
    };
  }
  return { outputs: null, acpOutputs: null, source: "none" };
}

function getOutputListRecency(list: OutputLine[]): number {
  if (list.length === 0) return 0;
  const last = list[list.length - 1];
  if (typeof last.ts === "number" && Number.isFinite(last.ts)) return last.ts;
  return 0;
}
