import { describe, expect, it } from "vitest";

import {
  shouldPersistRuntimeCacheFingerprint,
  shouldSkipRuntimeCacheSaveAfterHydrate,
} from "./runtime_cache_hydration";

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

describe("shouldPersistRuntimeCacheFingerprint", () => {
  it("persists when the fingerprint changes", () => {
    expect(shouldPersistRuntimeCacheFingerprint("before", "after")).toBe(true);
  });

  it("skips when the fingerprint is unchanged", () => {
    expect(shouldPersistRuntimeCacheFingerprint("same", "same")).toBe(false);
  });

  it("persists the first fingerprint", () => {
    expect(shouldPersistRuntimeCacheFingerprint(null, "first")).toBe(true);
  });
});
