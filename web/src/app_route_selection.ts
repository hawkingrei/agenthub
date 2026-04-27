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

export type WorkspaceLens = "channels" | "tasks" | "members" | "search" | "nodes";
export type TeamMemberRouteTab = "agent_acp" | "mailbox" | "member_console";

export function resolveWorkspaceLens(search: string): WorkspaceLens | null {
  const params = new URLSearchParams(search);
  const lens = params.get("lens");
  switch (lens) {
    case "channels":
    case "chat":
    case "threads":
      return "channels";
    case "tasks":
    case "members":
    case "search":
    case "nodes":
      return lens;
    default:
      return null;
  }
}

export function buildWorkspacePath(agentId?: string | null, lens?: WorkspaceLens | null): string {
  const pathname = agentId
    ? `/workspace/agents/${encodeURIComponent(agentId)}`
    : "/workspace";
  if (!lens) {
    return pathname;
  }
  return `${pathname}?lens=${encodeURIComponent(lens)}`;
}

export function resolveWorkspaceNodeId(search: string): string | null {
  const params = new URLSearchParams(search);
  const nodeId = params.get("node")?.trim() ?? "";
  return nodeId.length > 0 ? nodeId : null;
}

export function buildWorkspaceNodePath(nodeId?: string | null): string {
  const params = new URLSearchParams();
  params.set("lens", "nodes");
  if (nodeId?.trim()) {
    params.set("node", nodeId.trim());
  }
  return `/workspace?${params.toString()}`;
}

export function buildTeamDetailPath(teamId?: string | null): string {
  const normalizedTeamId = teamId?.trim();
  if (!normalizedTeamId) {
    return "/workspace/teams";
  }
  return `/workspace/teams/${encodeURIComponent(normalizedTeamId)}`;
}

export function buildTeamWorkspacePath(
  teamId?: string | null,
  lens?: WorkspaceLens | null,
  channelId?: string | null,
  threadRootMessageId?: number | null,
  memberId?: string | null,
  tab?: TeamMemberRouteTab | null
): string {
  const normalizedTeamId = teamId?.trim();
  if (!normalizedTeamId) {
    return "/workspace/teams";
  }
  const pathname = `/workspace/teams/${encodeURIComponent(normalizedTeamId)}`;
  const params = new URLSearchParams();
  if (lens) {
    params.set("lens", lens);
  }
  if (channelId && channelId !== "all") {
    params.set("channel", channelId);
  }
  if (threadRootMessageId && threadRootMessageId > 0) {
    params.set("thread", String(threadRootMessageId));
  }
  const normalizedMemberId = memberId?.trim() ?? "";
  if (normalizedMemberId) {
    params.set("member", normalizedMemberId);
  }
  if (tab === "agent_acp" || tab === "mailbox" || tab === "member_console") {
    params.set("tab", tab === "agent_acp" ? "thread" : tab);
  }
  const search = params.toString();
  return search ? `${pathname}?${search}` : pathname;
}

export function navigateToPath(pathname: string): void {
  const currentPath = `${window.location.pathname}${window.location.search}`;
  if (pathname === currentPath) {
    return;
  }
  window.history.pushState({}, "", pathname);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function shouldHandleInAppLinkClick(event: {
  button: number;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  defaultPrevented: boolean;
}): boolean {
  return !event.defaultPrevented && event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
}

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
