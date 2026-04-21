export function shouldSkipRuntimeCacheSaveAfterHydrate(
  pendingHydrationKey: string | null,
  cacheKey: string
): boolean {
  return cacheKey.length > 0 && pendingHydrationKey === cacheKey;
}
