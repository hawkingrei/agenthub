export type EventCursorSource = {
  ts: number;
  seq?: string;
};

export type EventCursor = { value: number | string; hasSeq: boolean };

export type CursorRef = { current: Record<string, EventCursor> };

export function getEventCursor(event: EventCursorSource): EventCursor {
  if (typeof event.seq === "string") {
    return { value: event.seq, hasSeq: true };
  }
  return { value: event.ts, hasSeq: false };
}

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
    if (cursor.hasSeq && !max.hasSeq) {
      max = cursor;
      continue;
    }
    if (cursor.hasSeq === max.hasSeq) {
      if (cursor.hasSeq) {
        if (String(cursor.value) > String(max.value)) {
          max = cursor;
        }
      } else if (Number(cursor.value) > Number(max.value)) {
        max = cursor;
      }
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
    (cursor.hasSeq && !prev.hasSeq) ||
    (cursor.hasSeq === prev.hasSeq &&
      (cursor.hasSeq
        ? String(cursor.value) > String(prev.value)
        : Number(cursor.value) > Number(prev.value)))
  ) {
    ref.current[key] = cursor;
  }
}

export function isCursorNewer(prev: EventCursor, next: EventCursor): boolean {
  if (next.hasSeq && !prev.hasSeq) return true;
  if (next.hasSeq !== prev.hasSeq) return false;
  if (next.hasSeq) {
    return String(next.value) > String(prev.value);
  }
  return Number(next.value) > Number(prev.value);
}

export function getAdaptivePollInterval(idleCount: number): number {
  const base = 2000;
  const max = 10000;
  if (idleCount <= 0) return base;
  return Math.min(max, base * (1 + idleCount));
}
