import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "../../storage/safe_storage";

// Only a retry identity is stored; authorization and execution remain server-side.
export function loopActivationIntentKey(
  userId: string,
  teamId: string,
  actorId: string,
): string {
  return `loop_activation_intent:${[userId, teamId, actorId].map(encodeURIComponent).join(":")}`;
}

export function readLoopActivationIntent(storageKey: string): string | null {
  const value = getLocalStorageItemSafe(storageKey);
  return value && /^[a-f0-9-]{36}$/i.test(value) ? value : null;
}

export function retainLoopActivationIntent(
  storageKey: string,
  intent: string,
): void {
  // The mounted controller also retains the identity when browser storage is unavailable.
  setLocalStorageItemSafe(storageKey, intent);
}

export function clearLoopActivationIntent(
  storageKey: string,
  intent: string,
): void {
  if (getLocalStorageItemSafe(storageKey) === intent)
    removeLocalStorageItemSafe(storageKey);
}
