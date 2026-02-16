import { removeLocalStorageItemSafe } from "./storage/safe_storage";

export function isInvalidTokenMessage(message: string | null): boolean {
  if (!message) return false;
  const lower = message.trim().toLowerCase();
  if (lower === "unauthorized") return true;
  if (lower.includes("invalid token")) return true;
  if (lower.includes("missing authorization token")) return true;
  return false;
}

export function shouldRedirectOnAuthError(
  status: number,
  token: string | null,
  message: string | null
): boolean {
  if (status !== 401) return false;
  if (!token) return false;
  return isInvalidTokenMessage(message);
}

export function clearAuthAndRedirect(): void {
  removeLocalStorageItemSafe("agenthub_auth");
  if (location.pathname === "/") {
    location.reload();
    return;
  }
  location.href = "/";
}
