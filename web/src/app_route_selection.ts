import { resolvePostLoginRedirectTarget } from "./auth_redirect";
import { AuthState } from "./types";

export type RouteLocationState = {
  pathname: string;
  search: string;
};

export type WorkspaceAgentRouteState =
  | {
      mode: "root";
      agentId: null;
    }
  | {
      mode: "agent";
      agentId: string;
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
  | "workspace";

export function isTeamsRoute(pathname: string): boolean {
  return (
    pathname === "/teams" ||
    pathname === "/teams/" ||
    pathname.startsWith("/teams/") ||
    pathname === "/workspace/teams" ||
    pathname === "/workspace/teams/" ||
    pathname.startsWith("/workspace/teams/")
  );
}

export function isWorkspaceRootRoute(pathname: string): boolean {
  return pathname === "/" || pathname === "/workspace" || pathname === "/workspace/";
}

export function isAgentsWorkbenchRoute(pathname: string): boolean {
  return isWorkspaceRootRoute(pathname) || pathname.startsWith("/workspace/agents/");
}

export function resolveTeamRoute(pathname: string): TeamRouteState | null {
  if (!isTeamsRoute(pathname)) {
    return null;
  }
  const prefix = pathname.startsWith("/workspace/teams") ? "/workspace/teams" : "/teams";
  const suffix = pathname.slice(prefix.length);
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

export function resolveWorkspaceAgentRoute(pathname: string): WorkspaceAgentRouteState | null {
  if (isWorkspaceRootRoute(pathname)) {
    return { mode: "root", agentId: null };
  }
  if (!pathname.startsWith("/workspace/agents/")) {
    return null;
  }
  const suffix = pathname.slice("/workspace/agents/".length);
  const [rawAgentId] = suffix.split("/");
  if (!rawAgentId) {
    return { mode: "root", agentId: null };
  }
  try {
    return {
      mode: "agent",
      agentId: decodeURIComponent(rawAgentId),
    };
  } catch {
    return {
      mode: "agent",
      agentId: rawAgentId,
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
  if (!isWorkspaceRootRoute(pathname)) return null;
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
  return "workspace";
}
