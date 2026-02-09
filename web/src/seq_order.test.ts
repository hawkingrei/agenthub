import { describe, expect, it } from "vitest";
import { compareEventOrder } from "./seq_order";

describe("compareEventOrder", () => {
  it("orders by event_id when present", () => {
    expect(compareEventOrder({ event_id: 10, ts: 1 }, { event_id: 11, ts: 1 })).toBeLessThan(0);
    expect(compareEventOrder({ event_id: 12, ts: 1 }, { event_id: 11, ts: 1 })).toBeGreaterThan(0);
  });

  it("prefers event_id over ts-only entries", () => {
    expect(compareEventOrder({ event_id: 1, ts: 1 }, { ts: 999 })).toBeGreaterThan(0);
    expect(compareEventOrder({ ts: 999 }, { event_id: 1, ts: 1 })).toBeLessThan(0);
  });

  it("falls back to ts when event_id is missing", () => {
    const left = { ts: 10 };
    const right = { ts: 20 };
    expect(compareEventOrder(left, right)).toBeLessThan(0);
    expect(compareEventOrder(right, left)).toBeGreaterThan(0);
  });

  it("orders null ts before non-null ts", () => {
    expect(compareEventOrder({ ts: null }, { ts: 1 })).toBeLessThan(0);
    expect(compareEventOrder({ ts: 1 }, { ts: null })).toBeGreaterThan(0);
  });

  it("returns 0 when event_id and ts are equal or missing", () => {
    expect(compareEventOrder({ ts: null }, { ts: null })).toBe(0);
    expect(compareEventOrder({ ts: 1 }, { ts: 1 })).toBe(0);
  });
});
