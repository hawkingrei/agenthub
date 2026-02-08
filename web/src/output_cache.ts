import { AgentEvent } from "./api";

export type OutputLine = AgentEvent;

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
  return deduped.sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
}

export function appendOutputLine(
  existing: OutputLine[],
  line: OutputLine
): OutputLine[] {
  if (existing.length === 0) return [line];
  const lineSeq = line.seq ?? 0;
  const lastSeq = existing[existing.length - 1].seq ?? 0;
  if (lineSeq >= lastSeq) {
    return [...existing, line];
  }
  const next = existing.slice();
  let lo = 0;
  let hi = next.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const midSeq = next[mid].seq ?? 0;
    if (midSeq <= lineSeq) {
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
