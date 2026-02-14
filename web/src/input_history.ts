export const INPUT_HISTORY_STORAGE_KEY = "agenthub_input_history";
export const DEFAULT_INPUT_HISTORY_LIMIT = 80;

const SENSITIVE_ASSIGNMENT_PATTERN =
  /(?:^|\s)(?:(?:--)?(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key)|(?:[a-z0-9_]*?(?:api[_-]?key|token|secret|password|passwd|private[_-]?key|access[_-]?key)))\s*[:=]\s*\S+/i;
const AUTHORIZATION_BEARER_PATTERN = /authorization\s*:\s*bearer\s+\S+/i;
const PRIVATE_KEY_PATTERN = /-----BEGIN\s+[^-]*PRIVATE KEY-----/i;

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
  if (!shouldStoreInputHistoryValue(normalized)) return history;
  const deduped = history.filter((item) => item !== normalized);
  return [normalized, ...deduped].slice(0, limit);
}

export function shouldStoreInputHistoryValue(value: string): boolean {
  if (SENSITIVE_ASSIGNMENT_PATTERN.test(value)) return false;
  if (AUTHORIZATION_BEARER_PATTERN.test(value)) return false;
  if (PRIVATE_KEY_PATTERN.test(value)) return false;
  return true;
}
