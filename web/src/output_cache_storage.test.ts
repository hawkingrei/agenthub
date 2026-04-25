import { beforeEach, afterEach, describe, expect, it } from "vitest";
import { loadOutputCaches, saveOutputCaches } from "./storage/output_cache_storage";
import { OutputLine } from "./output_cache";
import {
  DEFAULT_OUTPUT_CACHE_MAX_EVENTS,
  DEFAULT_OUTPUT_CACHE_MAX_SESSIONS,
} from "./storage/output_cache_budget";

const STORAGE_KEY = "agenthub_output_cache_v2";

class MemoryStorage {
  private store = new Map<string, string>();

  getItem(key: string) {
    return this.store.has(key) ? this.store.get(key)! : null;
  }

  setItem(key: string, value: string) {
    this.store.set(key, String(value));
  }

  removeItem(key: string) {
    this.store.delete(key);
  }

  clear() {
    this.store.clear();
  }
}

class QuotaFailOnceStorage extends MemoryStorage {
  private hasFailed = false;

  setItem(key: string, value: string) {
    if (!this.hasFailed) {
      this.hasFailed = true;
      const error = new Error("quota exceeded");
      (error as Error & { name: string }).name = "QuotaExceededError";
      throw error;
    }
    super.setItem(key, value);
  }
}

class AlwaysQuotaStorage extends MemoryStorage {
  setItem(_key: string, _value: string) {
    void _key;
    void _value;
    const error = new Error("quota exceeded");
    (error as Error & { name: string }).name = "QuotaExceededError";
    throw error;
  }
}

const makeLine = (
  event_id: number,
  ts: number,
  stream: OutputLine["stream"] = "stdout",
  sessionId = "session-1"
): OutputLine => ({
  agent_id: "agent-1",
  session_id: sessionId,
  ts,
  event_id,
  seq: String(event_id),
  stream,
  message: `${stream}-${event_id}`,
});

let originalStorage: unknown;

beforeEach(() => {
  originalStorage = (globalThis as { localStorage?: unknown }).localStorage;
  (globalThis as { localStorage?: unknown }).localStorage = new MemoryStorage();
});

afterEach(() => {
  if (originalStorage === undefined) {
    delete (globalThis as { localStorage?: unknown }).localStorage;
    return;
  }
  (globalThis as { localStorage?: unknown }).localStorage = originalStorage;
});

describe("output cache storage", () => {
  it("trims events per session", () => {
    const outputCache = {
      "agent-1:session-1": [
        makeLine(1, 10),
        makeLine(2, 20),
        makeLine(3, 30),
      ],
    };
    const acpOutputCache = {
      "agent-1:session-1": [makeLine(1, 11, "acp")],
    };
    saveOutputCaches(outputCache, acpOutputCache, 2, 5);

    const loaded = loadOutputCaches(2, 5);
    expect(loaded.outputCache["agent-1:session-1"].map((evt) => evt.event_id)).toEqual([
      2,
      3,
    ]);
    expect(loaded.acpOutputCache["agent-1:session-1"].length).toBe(1);
  });

  it("limits sessions by recency", () => {
    const outputCache = {
      "agent-1:session-1": [makeLine(1, 10, "stdout", "session-1")],
      "agent-1:session-2": [makeLine(2, 100, "stdout", "session-2")],
    };
    saveOutputCaches(outputCache, {}, 10, 1);

    const loaded = loadOutputCaches(10, 1);
    expect(Object.keys(loaded.outputCache)).toEqual(["agent-1:session-2"]);
  });

  it("returns empty caches on invalid payload", () => {
    const storage = (globalThis as unknown as { localStorage?: MemoryStorage }).localStorage;
    storage?.setItem(STORAGE_KEY, "not-json");

    const loaded = loadOutputCaches(10, 5);
    expect(loaded.outputCache).toEqual({});
    expect(loaded.acpOutputCache).toEqual({});
  });

  it("drops partially-shaped cached events", () => {
    const storage = (globalThis as unknown as { localStorage?: MemoryStorage }).localStorage;
    storage?.setItem(
      STORAGE_KEY,
      JSON.stringify({
        v: 1,
        updatedAt: Date.now(),
        outputCache: {
          "agent-1:session-1": [
            { stream: "stdout", ts: 1, message: "missing ids", seq: "1" },
            {
              agent_id: "agent-1",
              session_id: "session-1",
              stream: "stdout",
              ts: 2,
              message: "missing seq",
            },
            {
              agent_id: "agent-1",
              session_id: "session-1",
              seq: "3",
              stream: "bad",
              ts: 3,
              message: "bad stream",
            },
            makeLine(4, 4),
          ],
        },
        acpOutputCache: {},
      })
    );

    const loaded = loadOutputCaches(10, 5);
    expect(loaded.outputCache["agent-1:session-1"].map((evt) => evt.event_id)).toEqual([
      4,
    ]);
  });

  it("orders cached events by event_id even when ts is out of order", () => {
    const outputCache = {
      "agent-1:session-1": [makeLine(3, 1), makeLine(2, 999)],
    };
    saveOutputCaches(outputCache, {}, 10, 5);

    const loaded = loadOutputCaches(10, 5);
    expect(loaded.outputCache["agent-1:session-1"].map((evt) => evt.event_id)).toEqual([
      2,
      3,
    ]);
  });

  it("retries cache persistence after a quota error", () => {
    (globalThis as { localStorage?: unknown }).localStorage =
      new QuotaFailOnceStorage();
    const outputCache = {
      "agent-1:session-1": [makeLine(1, 10)],
    };
    const acpOutputCache = {
      "agent-1:session-1": [makeLine(2, 20, "acp")],
    };

    saveOutputCaches(outputCache, acpOutputCache, 10, 5);

    const loaded = loadOutputCaches(10, 5);
    expect(loaded.outputCache["agent-1:session-1"].length).toBe(1);
  });

  it("drops persisted cache when quota remains exceeded", () => {
    (globalThis as { localStorage?: unknown }).localStorage =
      new AlwaysQuotaStorage();
    const outputCache = {
      "agent-1:session-1": [makeLine(1, 10)],
    };

    expect(() => saveOutputCaches(outputCache, {}, 10, 5)).not.toThrow();
  });

  it("keeps persisted output cache within the default frontend memory budget", () => {
    const storage = (globalThis as unknown as { localStorage?: MemoryStorage }).localStorage;
    const messageBody = "x".repeat(160);
    const outputCache = Object.fromEntries(
      Array.from(
        { length: DEFAULT_OUTPUT_CACHE_MAX_SESSIONS + 6 },
        (_, sessionIndex) => {
          const sessionId = `session-${sessionIndex}`;
          const key = `agent-1:${sessionId}`;
          const lines = Array.from(
            { length: DEFAULT_OUTPUT_CACHE_MAX_EVENTS + 80 },
            (_, eventIndex) =>
              ({
                ...makeLine(
                  sessionIndex * 10_000 + eventIndex + 1,
                  eventIndex + 1,
                  "stdout",
                  sessionId
                ),
                message: `stdout-${sessionId}-${eventIndex}-${messageBody}`,
              }) satisfies OutputLine
          );
          return [key, lines] as const;
        }
      )
    );
    const acpOutputCache = Object.fromEntries(
      Array.from(
        { length: DEFAULT_OUTPUT_CACHE_MAX_SESSIONS + 4 },
        (_, sessionIndex) => {
          const sessionId = `acp-session-${sessionIndex}`;
          const key = `agent-1:${sessionId}`;
          const lines = Array.from(
            { length: DEFAULT_OUTPUT_CACHE_MAX_EVENTS + 80 },
            (_, eventIndex) =>
              ({
                ...makeLine(
                  100_000 + sessionIndex * 10_000 + eventIndex + 1,
                  eventIndex + 1,
                  "acp",
                  sessionId
                ),
                message: `acp-${sessionId}-${eventIndex}-${messageBody}`,
              }) satisfies OutputLine
          );
          return [key, lines] as const;
        }
      )
    );

    saveOutputCaches(
      outputCache,
      acpOutputCache,
      DEFAULT_OUTPUT_CACHE_MAX_EVENTS,
      DEFAULT_OUTPUT_CACHE_MAX_SESSIONS
    );

    const raw = storage?.getItem(STORAGE_KEY) ?? "";
    const parsed = raw ? JSON.parse(raw) : null;
    expect(Object.keys(parsed.outputCache)).toHaveLength(DEFAULT_OUTPUT_CACHE_MAX_SESSIONS);
    expect(Object.keys(parsed.acpOutputCache)).toHaveLength(DEFAULT_OUTPUT_CACHE_MAX_SESSIONS);
    expect(raw.length).toBeLessThan(2_000_000);
  });
});
