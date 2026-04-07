import { resolvePostLoginRedirectTarget } from "./auth_redirect";
import { AuthState } from "./types";

export type RouteLocationState = {
  pathname: string;
  search: string;
};

export type TeamRouteState =
  | {
      mode: "selector";
      teamId: null;
    }
  | {
      mode: "detail";
      teamId: string;
    };

export type AppRouteKind =
  | "join"
  | "admin-auth-required"
  | "admin-forbidden"
  | "admin"
  | "teams-auth-redirect"
  | "teams"
  | "post-auth-redirect"
  | "agents";

export function isTeamsRoute(pathname: string): boolean {
  return pathname === "/teams" || pathname === "/teams/" || pathname.startsWith("/teams/");
}

export function isAgentsWorkbenchRoute(pathname: string): boolean {
  return pathname === "/";
}

export function resolveTeamRoute(pathname: string): TeamRouteState | null {
  if (!isTeamsRoute(pathname)) {
    return null;
  }
  const suffix = pathname.slice("/teams".length);
  if (!suffix || suffix === "/") {
    return { mode: "selector", teamId: null };
  }
  const normalized = suffix.startsWith("/") ? suffix.slice(1) : suffix;
  const [rawTeamId] = normalized.split("/");
  if (!rawTeamId) {
    return { mode: "selector", teamId: null };
  }
  try {
    return {
      mode: "detail",
      teamId: decodeURIComponent(rawTeamId),
    };
  } catch {
    return {
      mode: "detail",
      teamId: rawTeamId,
    };
  }
}

export function shouldRedirectTeamsToLogin(
  pathname: string,
  auth: AuthState | null,
  token: string | null
): boolean {
  return isTeamsRoute(pathname) && (!auth || !token);
}

export function resolvePostAuthRedirectTarget(
  pathname: string,
  search: string,
  auth: AuthState | null,
  token: string | null
): string | null {
  if (pathname !== "/") return null;
  if (!auth || !token) return null;
  return resolvePostLoginRedirectTarget(search);
}

export function resolveAppRouteKind(
  location: RouteLocationState,
  auth: AuthState | null,
  token: string | null,
  postAuthRedirectTarget: string | null
): AppRouteKind {
  if (location.pathname.startsWith("/join")) {
    return "join";
  }
  if (location.pathname.startsWith("/admin")) {
    if (!auth) return "admin-auth-required";
    if (auth.role !== "root") return "admin-forbidden";
    return "admin";
  }
  if (isTeamsRoute(location.pathname)) {
    if (shouldRedirectTeamsToLogin(location.pathname, auth, token)) {
      return "teams-auth-redirect";
    }
    return "teams";
  }
  if (postAuthRedirectTarget) {
    return "post-auth-redirect";
  }
  return "agents";
}
