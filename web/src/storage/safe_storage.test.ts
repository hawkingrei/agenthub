import { afterEach, describe, expect, it, vi } from "vitest";
import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "./safe_storage";

const originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(
  globalThis,
  "localStorage"
);

function installMemoryLocalStorage() {
  const values = new Map<string, string>();
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
      removeItem: (key: string) => {
        values.delete(key);
      },
    },
  });
}

function restoreLocalStorage() {
  if (originalLocalStorageDescriptor) {
    Object.defineProperty(globalThis, "localStorage", originalLocalStorageDescriptor);
  } else {
    Reflect.deleteProperty(globalThis, "localStorage");
  }
}

describe("safe storage helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    restoreLocalStorage();
  });

  it("reads, writes, and removes localStorage values", () => {
    installMemoryLocalStorage();

    expect(setLocalStorageItemSafe("agenthub:test", "value")).toBe(true);
    expect(getLocalStorageItemSafe("agenthub:test")).toBe("value");

    removeLocalStorageItemSafe("agenthub:test");

    expect(getLocalStorageItemSafe("agenthub:test")).toBeNull();
  });

  it("returns fallback values when localStorage is unavailable", () => {
    Reflect.deleteProperty(globalThis, "localStorage");

    expect(getLocalStorageItemSafe("agenthub:test")).toBeNull();
    expect(setLocalStorageItemSafe("agenthub:test", "value")).toBe(false);
    expect(() => removeLocalStorageItemSafe("agenthub:test")).not.toThrow();
  });

  it("swallows localStorage access errors", () => {
    const storage = {
      getItem: vi.fn(() => {
        throw new Error("read failed");
      }),
      setItem: vi.fn(() => {
        throw new Error("write failed");
      }),
      removeItem: vi.fn(() => {
        throw new Error("remove failed");
      }),
    };
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: storage,
    });

    expect(getLocalStorageItemSafe("agenthub:test")).toBeNull();
    expect(setLocalStorageItemSafe("agenthub:test", "value")).toBe(false);
    expect(() => removeLocalStorageItemSafe("agenthub:test")).not.toThrow();
  });
});
