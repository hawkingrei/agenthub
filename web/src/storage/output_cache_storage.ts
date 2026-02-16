import { OutputLine } from "../output_cache";
import { compareEventOrder } from "../seq_order";

const STORAGE_KEY = "agenthub_output_cache_v2";
const FALLBACK_EVENTS_TIER1 = 80;
const FALLBACK_SESSIONS_TIER1 = 20;
const FALLBACK_EVENTS_TIER2 = 40;
const FALLBACK_SESSIONS_TIER2 = 10;

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
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(STORAGE_KEY);
  } catch {
    return { outputCache: {}, acpOutputCache: {} };
  }
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
  const attempts: ReadonlyArray<{
    events: number;
    sessions: number;
    dropAcp: boolean;
  }> = [
    { events: maxEvents, sessions: maxSessions, dropAcp: false },
    { events: maxEvents, sessions: maxSessions, dropAcp: true },
    {
      events: Math.min(maxEvents, FALLBACK_EVENTS_TIER1),
      sessions: Math.min(maxSessions, FALLBACK_SESSIONS_TIER1),
      dropAcp: false,
    },
    {
      events: Math.min(maxEvents, FALLBACK_EVENTS_TIER1),
      sessions: Math.min(maxSessions, FALLBACK_SESSIONS_TIER1),
      dropAcp: true,
    },
    {
      events: Math.min(maxEvents, FALLBACK_EVENTS_TIER2),
      sessions: Math.min(maxSessions, FALLBACK_SESSIONS_TIER2),
      dropAcp: true,
    },
  ];
  for (const attempt of attempts) {
    const payload: StoredOutputCache = {
      v: 1,
      updatedAt: Date.now(),
      outputCache: normalizeCache(outputCache, attempt.events, attempt.sessions),
      acpOutputCache: attempt.dropAcp
        ? {}
        : normalizeCache(acpOutputCache, attempt.events, attempt.sessions),
    };
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
      return;
    } catch (err) {
      if (!isQuotaExceededError(err)) return;
    }
  }
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore cleanup errors.
  }
}

function isQuotaExceededError(error: unknown): boolean {
  if (!error || typeof error !== "object") return false;
  const name = (error as { name?: string }).name;
  return name === "QuotaExceededError" || name === "NS_ERROR_DOM_QUOTA_REACHED";
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
  const sorted = [...filtered].sort((a, b) => compareEventOrder(a, b));
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
  if (typeof candidate.agent_id !== "string" || !candidate.agent_id) return false;
  if (typeof candidate.session_id !== "string" || !candidate.session_id) {
    return false;
  }
  const hasId = typeof candidate.event_id === "number";
  if (!hasId) {
    return false;
  }
  if (typeof candidate.seq !== "string" || !candidate.seq) {
    return false;
  }
  if (typeof candidate.message !== "string") return false;
  if (
    candidate.stream !== "stdout" &&
    candidate.stream !== "stderr" &&
    candidate.stream !== "system" &&
    candidate.stream !== "acp"
  ) {
    return false;
  }
  if (typeof candidate.ts !== "number") return false;
  return true;
}
