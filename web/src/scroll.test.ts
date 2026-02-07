import { describe, expect, it } from "vitest";
import { isNearBottom } from "./scroll";

describe("isNearBottom", () => {
  it("returns true when distance is below threshold", () => {
    expect(isNearBottom(1000, 880, 100)).toBe(true);
  });

  it("returns false when distance is above threshold", () => {
    expect(isNearBottom(1000, 600, 100)).toBe(false);
  });

  it("treats equality as not near", () => {
    expect(isNearBottom(1000, 780, 100, 120)).toBe(false);
  });
});
