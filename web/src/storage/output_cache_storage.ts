import { OutputLine } from "../output_cache";

const STORAGE_KEY = "agenthub_output_cache_v1";

type StoredOutputCache = {
  v: number;
  updatedAt: number;
  outputCache: Record<string, OutputLine[]>;
  acpOutputCache: Record<string, OutputLine[]>;
};

type LoadedCaches = {
  outputCache: Record<string, OutputLine[]>;
  acpOutputCache: Record<string, OutputLine[]>;
};

export function loadOutputCaches(
  maxEvents: number,
  maxSessions: number
): LoadedCaches {
  if (typeof localStorage === "undefined") {
    return { outputCache: {}, acpOutputCache: {} };
  }
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return { outputCache: {}, acpOutputCache: {} };
  try {
    const parsed = JSON.parse(raw) as StoredOutputCache;
    const outputCache = normalizeCache(parsed?.outputCache, maxEvents, maxSessions);
    const acpOutputCache = normalizeCache(
      parsed?.acpOutputCache,
      maxEvents,
      maxSessions
    );
    return { outputCache, acpOutputCache };
  } catch {
    return { outputCache: {}, acpOutputCache: {} };
  }
}

export function saveOutputCaches(
  outputCache: Record<string, OutputLine[]>,
  acpOutputCache: Record<string, OutputLine[]>,
  maxEvents: number,
  maxSessions: number
) {
  if (typeof localStorage === "undefined") return;
  const payload: StoredOutputCache = {
    v: 1,
    updatedAt: Date.now(),
    outputCache: normalizeCache(outputCache, maxEvents, maxSessions),
    acpOutputCache: normalizeCache(acpOutputCache, maxEvents, maxSessions),
  };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore storage errors (quota or permission).
  }
}

function normalizeCache(
  cache: Record<string, OutputLine[]> | undefined,
  maxEvents: number,
  maxSessions: number
): Record<string, OutputLine[]> {
  if (!cache) return {};
  const entries = Object.entries(cache)
    .map(([key, value]) => [key, sanitizeEvents(value, maxEvents)] as const)
    .filter(([, value]) => value.length > 0);

  const limitedEntries = limitSessions(entries, maxSessions);
  return Object.fromEntries(limitedEntries);
}

function sanitizeEvents(list: unknown, maxEvents: number): OutputLine[] {
  if (!Array.isArray(list)) return [];
  const filtered = list.filter(isOutputLine) as OutputLine[];
  if (filtered.length === 0) return [];
  const sorted = [...filtered].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
  if (sorted.length <= maxEvents) return sorted;
  return sorted.slice(sorted.length - maxEvents);
}

function limitSessions(
  entries: ReadonlyArray<readonly [string, OutputLine[]]>,
  maxSessions: number
): Array<readonly [string, OutputLine[]]> {
  if (entries.length <= maxSessions) return [...entries];
  return [...entries]
    .sort((a, b) => getLastTs(b[1]) - getLastTs(a[1]))
    .slice(0, maxSessions);
}

function getLastTs(events: OutputLine[]): number {
  if (events.length === 0) return 0;
  const last = events[events.length - 1];
  return typeof last.ts === "number" ? last.ts : 0;
}

function isOutputLine(value: unknown): value is OutputLine {
  if (!value || typeof value !== "object") return false;
  const candidate = value as OutputLine;
  if (typeof candidate.message !== "string") return false;
  if (typeof candidate.stream !== "string") return false;
  if (typeof candidate.ts !== "number") return false;
  return true;
}
