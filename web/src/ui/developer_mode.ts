import { getLocalStorageItemSafe, setLocalStorageItemSafe } from "../storage/safe_storage";

export const UI_PREFS_STORAGE_KEY = "agenthub_ui_prefs_v1";

type UiPreferences = {
  developerMode?: boolean;
};

function parseUiPreferences(raw: string | null): UiPreferences | null {
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return null;
    }
    return parsed as UiPreferences;
  } catch {
    return null;
  }
}

export function defaultDeveloperMode(isProd = import.meta.env.PROD): boolean {
  return !isProd;
}

export function loadDeveloperModePreference(isProd = import.meta.env.PROD): boolean {
  const stored = parseUiPreferences(getLocalStorageItemSafe(UI_PREFS_STORAGE_KEY));
  if (typeof stored?.developerMode === "boolean") {
    return stored.developerMode;
  }
  return defaultDeveloperMode(isProd);
}

export function persistDeveloperModePreference(value: boolean): void {
  const stored = parseUiPreferences(getLocalStorageItemSafe(UI_PREFS_STORAGE_KEY));
  const next: UiPreferences = {
    ...(stored ?? {}),
    developerMode: value,
  };
  setLocalStorageItemSafe(UI_PREFS_STORAGE_KEY, JSON.stringify(next));
}
