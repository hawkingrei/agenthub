import { describe, expect, it } from "vitest";
import {
  getAdaptivePollInterval,
  getMaxEventCursor,
  isSseConnectionStale,
  isCursorNewer,
  shouldPollAgentEvents,
  updateLastEventCursor,
} from "./event_polling";

describe("getAdaptivePollInterval", () => {
  it("returns the base interval when idleCount is zero or negative", () => {
    expect(getAdaptivePollInterval(0)).toBe(2000);
    expect(getAdaptivePollInterval(-1)).toBe(2000);
  });

  it("grows linearly and caps at the max interval", () => {
    expect(getAdaptivePollInterval(1)).toBe(4000);
    expect(getAdaptivePollInterval(2)).toBe(6000);
    expect(getAdaptivePollInterval(3)).toBe(8000);
    expect(getAdaptivePollInterval(4)).toBe(10000);
    expect(getAdaptivePollInterval(10)).toBe(10000);
  });
});

describe("shouldPollAgentEvents", () => {
  it("polls when SSE is not open", () => {
    expect(shouldPollAgentEvents(false, null, 1000)).toBe(true);
  });

  it("polls during boost window even when SSE is open", () => {
    expect(shouldPollAgentEvents(true, 5000, 1000)).toBe(true);
  });

  it("skips polling when SSE is open and boost window expired", () => {
    expect(shouldPollAgentEvents(true, 1000, 1000)).toBe(false);
    expect(shouldPollAgentEvents(true, null, 1000)).toBe(false);
  });

  it("polls when SSE connection is stale", () => {
    expect(shouldPollAgentEvents(true, null, 1000, true)).toBe(true);
  });
});

describe("isSseConnectionStale", () => {
  it("returns false when SSE is not open", () => {
    expect(isSseConnectionStale(false, 0, 1000, 30_000)).toBe(false);
  });

  it("returns true when SSE idle time exceeds stale threshold", () => {
    expect(isSseConnectionStale(true, 0, 31_000, 30_000)).toBe(true);
  });

  it("returns false when SSE idle time is below stale threshold", () => {
    expect(isSseConnectionStale(true, 2_000, 31_000, 30_000)).toBe(false);
  });
});

describe("event cursor helpers", () => {
  it("uses event_id when present and falls back to ts", () => {
    const max = getMaxEventCursor([
      { ts: 10 },
      { ts: 40, event_id: 22 },
      { ts: 40 },
    ]);
    expect(max?.value).toBe(22);
    expect(max?.kind).toBe("event_id");
  });

  it("returns null when given an empty list", () => {
    expect(getMaxEventCursor([])).toBeNull();
  });

  it("updates cursor only when the new value is larger", () => {
    const ref = {
      current: {} as Record<
        string,
        { value: number; kind: "event_id" | "ts" }
      >,
    };
    updateLastEventCursor(ref, "a", { ts: 10 });
    expect(ref.current.a?.value).toBe(10);
    updateLastEventCursor(ref, "a", { ts: 20 });
    expect(ref.current.a?.value).toBe(20);
    updateLastEventCursor(ref, "a", { ts: 15 });
    expect(ref.current.a?.value).toBe(20);
  });

  it("prefers event_id over ts-only events", () => {
    const max = getMaxEventCursor([
      { ts: 999 },
      { ts: 10, event_id: 1 },
      { ts: 5, event_id: 2 },
    ]);
    expect(max?.kind).toBe("event_id");
    expect(max?.value).toBe(2);
  });

  it("treats event_id cursor as newer than ts cursor", () => {
    const prev = { value: 100, kind: "ts" } as const;
    const next = { value: 1, kind: "event_id" } as const;
    expect(isCursorNewer(prev, next)).toBe(true);
    expect(isCursorNewer(next, prev)).toBe(false);
  });

  it("does not update when event_id regresses even if ts increases", () => {
    const ref = {
      current: {} as Record<
        string,
        { value: number; kind: "event_id" | "ts" }
      >,
    };
    updateLastEventCursor(ref, "a", { ts: 10, event_id: 5 });
    updateLastEventCursor(ref, "a", { ts: 99, event_id: 4 });
    expect(ref.current.a?.value).toBe(5);
    expect(ref.current.a?.kind).toBe("event_id");
  });
});
