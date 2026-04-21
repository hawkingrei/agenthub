import { describe, expect, it } from "vitest";

import { shouldSkipRuntimeCacheSaveAfterHydrate } from "./runtime_cache_hydration";

describe("shouldSkipRuntimeCacheSaveAfterHydrate", () => {
  it("skips the first save for the just-hydrated cache key", () => {
    expect(
      shouldSkipRuntimeCacheSaveAfterHydrate("team-1:all", "team-1:all")
    ).toBe(true);
  });

  it("does not skip saves for other cache keys", () => {
    expect(
      shouldSkipRuntimeCacheSaveAfterHydrate("team-1:all", "team-1:other")
    ).toBe(false);
  });

  it("does not skip when there is no pending hydration key", () => {
    expect(
      shouldSkipRuntimeCacheSaveAfterHydrate(null, "team-1:all")
    ).toBe(false);
  });
});
