import { describe, expect, it } from "vitest";
import {
  getAdaptivePollInterval,
  getMaxEventCursor,
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

describe("event cursor helpers", () => {
  it("uses seq when present and falls back to ts", () => {
    const max = getMaxEventCursor([
      { ts: 10 },
      { ts: 20, seq: 1 },
      { ts: 30, seq: 5 },
      { ts: 40 },
    ]);
    expect(max?.value).toBe(5);
    expect(max?.hasSeq).toBe(true);
  });

  it("returns null when given an empty list", () => {
    expect(getMaxEventCursor([])).toBeNull();
  });

  it("updates cursor only when the new value is larger", () => {
    const ref = { current: {} as Record<string, { value: number; hasSeq: boolean }> };
    updateLastEventCursor(ref, "a", { ts: 10, seq: 5 });
    expect(ref.current.a?.value).toBe(5);
    updateLastEventCursor(ref, "a", { ts: 20, seq: 4 });
    expect(ref.current.a?.value).toBe(5);
    updateLastEventCursor(ref, "a", { ts: 30, seq: 7 });
    expect(ref.current.a?.value).toBe(7);
  });
});
