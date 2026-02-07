export type EventCursorSource = {
  ts: number;
  seq?: number;
};

export type CursorRef = { current: Record<string, number> };

export function getEventCursor(event: EventCursorSource): number {
  return event.seq ?? event.ts;
}

export function getMaxEventCursor(events: EventCursorSource[]): number | null {
  let max: number | null = null;
  for (const evt of events) {
    const cursor = getEventCursor(evt);
    if (max == null || cursor > max) {
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
  if (prev == null || cursor > prev) {
    ref.current[key] = cursor;
  }
}

export function getAdaptivePollInterval(idleCount: number): number {
  const base = 2000;
  const max = 10000;
  if (idleCount <= 0) return base;
  return Math.min(max, base * (1 + idleCount));
}
