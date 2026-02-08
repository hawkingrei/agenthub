import { describe, expect, it } from "vitest";
import { compareEventOrder, compareSeqValue } from "./seq_order";

describe("compareSeqValue", () => {
  it("orders numeric strings by length then lexicographic", () => {
    expect(compareSeqValue("9", "10")).toBeLessThan(0);
    expect(compareSeqValue("10", "9")).toBeGreaterThan(0);
    expect(compareSeqValue("100", "100")).toBe(0);
  });

  it("orders UUIDv7 strings lexicographically", () => {
    const left = "018f1f7e-7f6a-7000-8000-000000000000";
    const right = "018f1f7e-7f6a-7001-8000-000000000000";
    expect(compareSeqValue(left, right)).toBeLessThan(0);
  });

  it("returns null when seq types differ", () => {
    const numeric = "123";
    const uuid = "018f1f7e-7f6a-7000-8000-000000000000";
    expect(compareSeqValue(numeric, uuid)).toBeNull();
  });
});

describe("compareEventOrder", () => {
  it("falls back to ts when seq types differ", () => {
    const numeric = { seq: "123", ts: 10 };
    const uuid = { seq: "018f1f7e-7f6a-7000-8000-000000000000", ts: 20 };
    expect(compareEventOrder(numeric, uuid)).toBeLessThan(0);
    expect(compareEventOrder(uuid, numeric)).toBeGreaterThan(0);
  });

  it("returns 0 when both seq and ts are equal or missing", () => {
    expect(compareEventOrder({ seq: null, ts: null }, { seq: null, ts: null })).toBe(0);
    expect(compareEventOrder({ seq: "1", ts: 1 }, { seq: "1", ts: 1 })).toBe(0);
  });
});
