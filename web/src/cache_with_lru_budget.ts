export function cacheWithLruBudget<K, V>(
  cache: Map<K, V>,
  sizes: Map<K, number>,
  currentBytes: () => number,
  setBytes: (next: number) => void,
  key: K,
  value: V,
  size: number,
  entryLimit: number,
  byteLimit: number
): V {
  if (cache.has(key)) {
    const previousSize = sizes.get(key) ?? 0;
    setBytes(Math.max(0, currentBytes() - previousSize));
    sizes.delete(key);
    cache.delete(key);
  }
  cache.set(key, value);
  sizes.set(key, size);
  setBytes(currentBytes() + size);
  while (cache.size > entryLimit || currentBytes() > byteLimit) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey !== undefined) {
      const oldestSize = sizes.get(oldestKey) ?? 0;
      setBytes(Math.max(0, currentBytes() - oldestSize));
      sizes.delete(oldestKey);
      cache.delete(oldestKey);
      continue;
    }
    break;
  }
  return value;
}

export function refreshCacheRecency<K, V>(
  cache: Map<K, V>,
  sizes: Map<K, number>,
  key: K
): void {
  const value = cache.get(key);
  if (value == null) {
    return;
  }
  const size = sizes.get(key);
  cache.delete(key);
  cache.set(key, value);
  if (size != null) {
    sizes.delete(key);
    sizes.set(key, size);
  }
}
