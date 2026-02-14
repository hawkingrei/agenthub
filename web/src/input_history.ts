export const INPUT_HISTORY_STORAGE_KEY = "agenthub_input_history";
export const DEFAULT_INPUT_HISTORY_LIMIT = 80;

export function parseInputHistory(
  raw: string | null,
  limit: number = DEFAULT_INPUT_HISTORY_LIMIT
): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const normalized = parsed
      .map((item) => (typeof item === "string" ? item.trim() : ""))
      .filter((item) => item.length > 0);
    const deduped: string[] = [];
    for (const item of normalized) {
      if (deduped.includes(item)) continue;
      deduped.push(item);
      if (deduped.length >= limit) break;
    }
    return deduped;
  } catch {
    return [];
  }
}

export function pushInputHistory(
  history: string[],
  value: string,
  limit: number = DEFAULT_INPUT_HISTORY_LIMIT
): string[] {
  const normalized = value.trim();
  if (!normalized) return history;
  const deduped = history.filter((item) => item !== normalized);
  return [normalized, ...deduped].slice(0, limit);
}
