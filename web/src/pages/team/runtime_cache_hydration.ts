export function shouldSkipRuntimeCacheSaveAfterHydrate(
  pendingHydrationKey: string | null,
  cacheKey: string
): boolean {
  return cacheKey.length > 0 && pendingHydrationKey === cacheKey;
}

export function shouldPersistRuntimeCacheFingerprint(
  previousFingerprint: string | null,
  nextFingerprint: string
): boolean {
  return previousFingerprint !== nextFingerprint;
}
