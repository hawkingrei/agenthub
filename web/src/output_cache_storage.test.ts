import { beforeEach, afterEach, describe, expect, it } from "vitest";
import { loadOutputCaches, saveOutputCaches } from "./storage/output_cache_storage";
import { OutputLine } from "./output_cache";

const STORAGE_KEY = "agenthub_output_cache_v1";

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

const makeLine = (
  seq: number,
  ts: number,
  stream: OutputLine["stream"] = "stdout",
  sessionId = "session-1"
): OutputLine => ({
  agent_id: "agent-1",
  session_id: sessionId,
  ts,
  seq,
  stream,
  message: `${stream}-${seq}`,
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
    expect(loaded.outputCache["agent-1:session-1"].map((evt) => evt.seq)).toEqual([
      2,
      3,
    ]);
    expect(loaded.acpOutputCache["agent-1:session-1"].length).toBe(1);
  });

  it("limits sessions by recency", () => {
    const outputCache = {
      "agent-1:session-1": [makeLine(1, 10, "stdout", "session-1")],
      "agent-1:session-2": [makeLine(1, 100, "stdout", "session-2")],
    };
    saveOutputCaches(outputCache, {}, 10, 1);

    const loaded = loadOutputCaches(10, 1);
    expect(Object.keys(loaded.outputCache)).toEqual(["agent-1:session-2"]);
  });

  it("returns empty caches on invalid payload", () => {
    const storage = (globalThis as { localStorage?: MemoryStorage }).localStorage;
    storage?.setItem(STORAGE_KEY, "not-json");

    const loaded = loadOutputCaches(10, 5);
    expect(loaded.outputCache).toEqual({});
    expect(loaded.acpOutputCache).toEqual({});
  });

  it("drops partially-shaped cached events", () => {
    const storage = (globalThis as { localStorage?: MemoryStorage }).localStorage;
    storage?.setItem(
      STORAGE_KEY,
      JSON.stringify({
        v: 1,
        updatedAt: Date.now(),
        outputCache: {
          "agent-1:session-1": [
            { stream: "stdout", ts: 1, message: "missing ids", seq: 1 },
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
              seq: 3,
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
    expect(loaded.outputCache["agent-1:session-1"].map((evt) => evt.seq)).toEqual([
      4,
    ]);
  });
});
