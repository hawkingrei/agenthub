// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  defaultDeveloperMode,
  loadDeveloperModePreference,
  persistDeveloperModePreference,
} from "./developer_mode";

describe("developer mode preferences", () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
      removeItem: (key: string) => {
        store.delete(key);
      },
      clear: () => {
        store.clear();
      },
    });
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it("defaults to enabled outside production", () => {
    expect(defaultDeveloperMode(false)).toBe(true);
    expect(loadDeveloperModePreference(false)).toBe(true);
  });

  it("defaults to disabled in production", () => {
    expect(defaultDeveloperMode(true)).toBe(false);
    expect(loadDeveloperModePreference(true)).toBe(false);
  });

  it("uses stored preference when present", () => {
    persistDeveloperModePreference(false);
    expect(loadDeveloperModePreference(false)).toBe(false);

    persistDeveloperModePreference(true);
    expect(loadDeveloperModePreference(true)).toBe(true);
  });

  it("falls back to environment default when stored data is malformed", () => {
    localStorage.setItem("agenthub_ui_prefs_v1", "{bad json");
    expect(loadDeveloperModePreference(true)).toBe(false);
    expect(loadDeveloperModePreference(false)).toBe(true);
  });
});
