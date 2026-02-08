import { AgentEvent } from "./api";

export type OutputLine = AgentEvent;

const compareOutputLines = (a: OutputLine, b: OutputLine): number => {
  const aSeq = a.seq;
  const bSeq = b.seq;
  if (aSeq != null && bSeq != null) {
    if (aSeq === bSeq) return 0;
    return aSeq < bSeq ? -1 : 1;
  }
  if (aSeq != null) return 1;
  if (bSeq != null) return -1;
  if (a.ts !== b.ts) return a.ts - b.ts;
  if (a.stream !== b.stream) return a.stream < b.stream ? -1 : 1;
  if (a.message !== b.message) return a.message < b.message ? -1 : 1;
  return 0;
};

export function mergeOutputs(
  existing: OutputLine[],
  incoming: OutputLine[]
): OutputLine[] {
  const merged = [...existing, ...incoming];
  const seen = new Set<string>();
  const deduped: OutputLine[] = [];
  for (const line of merged) {
    const key =
      line.seq != null
        ? String(line.seq)
        : `${line.ts}-${line.stream}-${line.message}`;
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.push(line);
  }
  return deduped.sort(compareOutputLines);
}

export function appendOutputLine(
  existing: OutputLine[],
  line: OutputLine
): OutputLine[] {
  if (existing.length === 0) return [line];
  const last = existing[existing.length - 1];
  if (compareOutputLines(last, line) <= 0) {
    return [...existing, line];
  }
  const next = existing.slice();
  let lo = 0;
  let hi = next.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (compareOutputLines(next[mid], line) <= 0) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  next.splice(lo, 0, line);
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
    (aFirst.seq ?? null) === (bFirst.seq ?? null) &&
    (aLast.seq ?? null) === (bLast.seq ?? null) &&
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
