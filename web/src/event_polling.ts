export type EventCursorSource = {
  ts: number;
  event_id?: number;
};

export type EventCursor = {
  value: number;
  kind: "event_id" | "ts";
};

export type CursorRef = { current: Record<string, EventCursor> };

export function getEventCursor(event: EventCursorSource): EventCursor {
  if (typeof event.event_id === "number") {
    return { value: event.event_id, kind: "event_id" };
  }
  return { value: event.ts, kind: "ts" };
}

const cursorPriority = (kind: EventCursor["kind"]): number => {
  if (kind === "event_id") return 1;
  return 0;
};

const isCursorValueGreater = (left: EventCursor, right: EventCursor): boolean => {
  if (left.kind !== right.kind) return false;
  return left.value > right.value;
};

export function getMaxEventCursor(
  events: EventCursorSource[]
): EventCursor | null {
  let max: EventCursor | null = null;
  for (const evt of events) {
    const cursor = getEventCursor(evt);
    if (max == null) {
      max = cursor;
      continue;
    }
    const cursorRank = cursorPriority(cursor.kind);
    const maxRank = cursorPriority(max.kind);
    if (cursorRank > maxRank) {
      max = cursor;
      continue;
    }
    if (cursorRank === maxRank && isCursorValueGreater(cursor, max)) {
      max = cursor;
    }
  }
  return max;
}

export function updateLastEventCursor(
  ref: CursorRef,
  key: string,
  event: EventCursorSource
): void {
  const cursor = getEventCursor(event);
  const prev = ref.current[key];
  if (
    prev == null ||
    cursorPriority(cursor.kind) > cursorPriority(prev.kind) ||
    (cursor.kind === prev.kind && isCursorValueGreater(cursor, prev))
  ) {
    ref.current[key] = cursor;
  }
}

export function isCursorNewer(prev: EventCursor, next: EventCursor): boolean {
  const prevRank = cursorPriority(prev.kind);
  const nextRank = cursorPriority(next.kind);
  if (nextRank > prevRank) return true;
  if (nextRank < prevRank) return false;
  return isCursorValueGreater(next, prev);
}

export function getAdaptivePollInterval(idleCount: number): number {
  const base = 2000;
  const max = 10000;
  if (idleCount <= 0) return base;
  return Math.min(max, base * (1 + idleCount));
}
