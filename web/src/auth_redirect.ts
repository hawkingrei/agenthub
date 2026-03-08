import { removeLocalStorageItemSafe } from "./storage/safe_storage";

const POST_LOGIN_REDIRECT_PARAM = "next";

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

export function normalizePostLoginRedirectTarget(target: string | null | undefined): string | null {
  if (!target) return null;
  const normalized = target.trim();
  if (!normalized || normalized === "/") return null;
  if (!normalized.startsWith("/")) return null;
  if (normalized.startsWith("//")) return null;
  return normalized;
}

export function buildLoginRedirectPath(target: string | null | undefined): string {
  const normalized = normalizePostLoginRedirectTarget(target);
  if (!normalized) return "/";
  const params = new URLSearchParams([[POST_LOGIN_REDIRECT_PARAM, normalized]]);
  return `/?${params.toString()}`;
}

export function resolvePostLoginRedirectTarget(search: string): string | null {
  return normalizePostLoginRedirectTarget(
    new URLSearchParams(search).get(POST_LOGIN_REDIRECT_PARAM)
  );
}

export function clearAuthAndRedirect(target?: string | null): void {
  removeLocalStorageItemSafe("agenthub_auth");
  const redirectPath = buildLoginRedirectPath(target);
  const currentPath = `${location.pathname}${location.search}`;
  if (currentPath === redirectPath && !location.hash) {
    location.reload();
    return;
  }
  location.href = redirectPath;
}
