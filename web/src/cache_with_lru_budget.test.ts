import { describe, expect, it } from "vitest";
import { cacheWithLruBudget, refreshCacheRecency } from "./cache_with_lru_budget";

describe("cacheWithLruBudget", () => {
  it("evicts the oldest entry when entry limit is exceeded", () => {
    const cache = new Map<string, string>();
    const sizes = new Map<string, number>();
    let bytes = 0;

    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "a", "value-a", 2, 2, 10);
    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "b", "value-b", 2, 2, 10);
    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "c", "value-c", 2, 2, 10);

    expect([...cache.keys()]).toEqual(["b", "c"]);
    expect(bytes).toBe(4);
  });

  it("refreshes recency before applying LRU eviction", () => {
    const cache = new Map<string, string>();
    const sizes = new Map<string, number>();
    let bytes = 0;

    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "a", "value-a", 2, 2, 10);
    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "b", "value-b", 2, 2, 10);

    refreshCacheRecency(cache, sizes, "a");

    cacheWithLruBudget(cache, sizes, () => bytes, (next) => {
      bytes = next;
    }, "c", "value-c", 2, 2, 10);

    expect([...cache.keys()]).toEqual(["a", "c"]);
    expect(bytes).toBe(4);
  });
});
