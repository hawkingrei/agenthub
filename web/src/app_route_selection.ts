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

export type WorkspaceNodeRouteState =
  | {
      mode: "list";
      nodeId: null;
    }
  | {
      mode: "detail";
      nodeId: string;
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

export type WorkspaceLens = "teams" | "channels" | "tasks" | "members" | "search" | "nodes";
export type TeamMemberRouteTab = "agent_acp" | "mailbox" | "member_console";
const TEAM_MEMBER_ROUTE_TABS = new Set<TeamMemberRouteTab>([
  "agent_acp",
  "mailbox",
  "member_console",
]);

export function isTeamMemberRouteTab(value: string | null | undefined): value is TeamMemberRouteTab {
  return value != null && TEAM_MEMBER_ROUTE_TABS.has(value as TeamMemberRouteTab);
}

/**
 * Resolves the logical lens for the current workspace view.
 * Path-aware defaults:
 * - root / agents -> channels
 * - selector -> teams
 * - nodes -> nodes
 */
export function resolveWorkspaceLens(pathname: string, search: string): WorkspaceLens {
  const params = new URLSearchParams(search);
  const lens = params.get("lens");

  if (isTeamsRoute(pathname)) {
    const route = resolveTeamRoute(pathname);
    if (route?.mode === "selector") {
      return "teams";
    }
    if (lens === "tasks" || lens === "members") {
      return lens;
    }
    return "channels";
  }

  if (lens === "channels" || lens === "chat" || lens === "threads") return "channels";
  if (lens === "tasks" || lens === "members" || lens === "search" || lens === "nodes") return lens as WorkspaceLens;
  if (lens === "teams") return "teams";

  if (pathname.startsWith("/workspace/nodes")) return "nodes";

  return "channels";
}

export function buildWorkspacePath(agentId?: string | null, lens?: WorkspaceLens | null): string {
  const pathname = agentId
    ? `/workspace/agents/${encodeURIComponent(agentId)}`
    : "/workspace";
  // Omit default lens for root/agents to keep URLs clean
  if (!lens || lens === "channels") {
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
  const normalizedNodeId = nodeId?.trim() ?? "";
  if (!normalizedNodeId) {
    return "/workspace/nodes";
  }
  return `/workspace/nodes/${encodeURIComponent(normalizedNodeId)}`;
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
  tab?: TeamMemberRouteTab | null,
  taskId?: string | null
): string {
  const normalizedTeamId = teamId?.trim();
  if (!normalizedTeamId) {
    return "/workspace/teams";
  }
  const pathname = `/workspace/teams/${encodeURIComponent(normalizedTeamId)}`;
  const params = new URLSearchParams();
  // Omit default lens for team detail to keep URLs clean
  if (lens && lens !== "teams" && lens !== "channels" && lens !== "search") {
    params.set("lens", lens);
  }
  if (channelId && channelId !== "all") {
    params.set("channel", channelId);
  }
  if (threadRootMessageId && threadRootMessageId > 0) {
    params.set("thread", String(threadRootMessageId));
  }
  const normalizedTaskId = taskId?.trim() ?? "";
  if (normalizedTaskId) {
    params.set("task", normalizedTaskId);
  }
  const normalizedMemberId = memberId?.trim() ?? "";
  if (normalizedMemberId) {
    params.set("member", normalizedMemberId);
  }
  if (isTeamMemberRouteTab(tab)) {
    params.set("tab", tab === "agent_acp" ? "thread" : tab);
  }
  const search = params.toString();
  return search ? `${pathname}?${search}` : pathname;
}

export function resolveTeamSelectedTaskId(search: string): string {
  const params = new URLSearchParams(search);
  return (params.get("task") ?? "").trim();
}

export function resolveTeamMemberRouteTab(search: string): TeamMemberRouteTab | null {
  const params = new URLSearchParams(search);
  const raw = (params.get("tab") ?? "").trim();
  if (raw === "thread") {
    return "agent_acp";
  }
  return isTeamMemberRouteTab(raw) ? raw : null;
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

export function isWorkspaceWorkbenchRoute(pathname: string): boolean {
  return (
    isWorkspaceRootRoute(pathname) ||
    pathname.startsWith("/workspace/agents/") ||
    pathname === "/workspace/nodes" ||
    pathname === "/workspace/nodes/" ||
    pathname.startsWith("/workspace/nodes/")
  );
}

export function isAgentsWorkbenchRoute(pathname: string): boolean {
  return isWorkspaceWorkbenchRoute(pathname);
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

export function resolveWorkspaceNodeRoute(pathname: string, search: string): WorkspaceNodeRouteState | null {
  if (pathname === "/workspace/nodes" || pathname === "/workspace/nodes/") {
    return { mode: "list", nodeId: null };
  }
  if (pathname.startsWith("/workspace/nodes/")) {
    const suffix = pathname.slice("/workspace/nodes/".length);
    const [rawNodeId] = suffix.split("/");
    if (!rawNodeId) {
      return { mode: "list", nodeId: null };
    }
    try {
      return {
        mode: "detail",
        nodeId: decodeURIComponent(rawNodeId),
      };
    } catch {
      return {
        mode: "detail",
        nodeId: rawNodeId,
      };
    }
  }
  const legacyNodeId = resolveWorkspaceNodeId(search);
  const legacyLens = resolveWorkspaceLens(pathname, search);
  if (isWorkspaceRootRoute(pathname) && legacyLens === "nodes") {
    return legacyNodeId
      ? { mode: "detail", nodeId: legacyNodeId }
      : { mode: "list", nodeId: null };
  }
  return null;
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
    if (!auth || !token) return "admin-auth-required";
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
