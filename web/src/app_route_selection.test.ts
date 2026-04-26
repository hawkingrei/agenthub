// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { AuthState } from "./types";
import {
  buildWorkspaceNodePath,
  buildWorkspacePath,
  navigateToPath,
  resolveAppRouteKind,
  resolvePostAuthRedirectTarget,
  resolveTeamRoute,
  resolveWorkspaceLens,
  resolveWorkspaceAgentRoute,
  resolveWorkspaceNodeId,
  shouldRedirectTeamsToLogin,
  type RouteLocationState,
} from "./app_route_selection";

const rootAuth: AuthState = {
  token: "token-1",
  userId: "user-1",
  username: "root",
  role: "root",
};

function location(pathname: string, search = ""): RouteLocationState {
  return { pathname, search };
}

describe("app route selection", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    window.history.replaceState({}, "", "/");
  });

  it("redirects unauthenticated team routes to login", () => {
    expect(shouldRedirectTeamsToLogin("/teams", null, null)).toBe(true);
    expect(shouldRedirectTeamsToLogin("/workspace/teams", null, null)).toBe(true);
    expect(shouldRedirectTeamsToLogin("/teams/team-1", rootAuth, "token-1")).toBe(false);
    expect(shouldRedirectTeamsToLogin("/admin", null, null)).toBe(false);
  });

  it("resolves team selector and detail routes", () => {
    expect(resolveTeamRoute("/teams")).toEqual({ mode: "selector", teamId: null });
    expect(resolveTeamRoute("/workspace/teams")).toEqual({
      mode: "selector",
      teamId: null,
    });
    expect(resolveTeamRoute("/teams/team-1")).toEqual({
      mode: "detail",
      teamId: "team-1",
    });
    expect(resolveTeamRoute("/workspace/teams/team-1")).toEqual({
      mode: "detail",
      teamId: "team-1",
    });
    expect(resolveTeamRoute("/teams/team%2F1")).toEqual({
      mode: "detail",
      teamId: "team/1",
    });
    expect(resolveTeamRoute("/agents")).toBeNull();
  });

  it("resolves canonical workspace agent routes", () => {
    expect(resolveWorkspaceAgentRoute("/workspace")).toEqual({
      mode: "root",
      agentId: null,
    });
    expect(resolveWorkspaceAgentRoute("/workspace/agents/agent-1")).toEqual({
      mode: "agent",
      agentId: "agent-1",
    });
    expect(resolveWorkspaceAgentRoute("/workspace/agents/agent%2F1")).toEqual({
      mode: "agent",
      agentId: "agent/1",
    });
    expect(resolveWorkspaceAgentRoute("/teams/team-1")).toBeNull();
  });

  it("maps legacy chat and threads lens values to channels", () => {
    expect(resolveWorkspaceLens("?lens=channels")).toBe("channels");
    expect(resolveWorkspaceLens("?lens=chat")).toBe("channels");
    expect(resolveWorkspaceLens("?lens=threads")).toBe("channels");
    expect(resolveWorkspaceLens("?lens=nodes")).toBe("nodes");
    expect(resolveWorkspaceLens("?lens=unknown")).toBe(null);
    expect(buildWorkspacePath("agent-1", "channels")).toBe(
      "/workspace/agents/agent-1?lens=channels"
    );
    expect(resolveWorkspaceNodeId("?lens=nodes&node=node-east")).toBe("node-east");
    expect(resolveWorkspaceNodeId("?lens=nodes")).toBeNull();
    expect(buildWorkspaceNodePath("node-east")).toBe("/workspace?lens=nodes&node=node-east");
  });

  it("derives the post-auth redirect target only on the workspace root aliases", () => {
    expect(
      resolvePostAuthRedirectTarget(
        "/",
        "?next=%2Fteams%3Ftab%3Druns%23active",
        rootAuth,
        "token-1"
      )
    ).toBe("/teams?tab=runs#active");
    expect(
      resolvePostAuthRedirectTarget(
        "/workspace",
        "?next=%2Fteams%3Ftab%3Druns%23active",
        rootAuth,
        "token-1"
      )
    ).toBe("/teams?tab=runs#active");
    expect(resolvePostAuthRedirectTarget("/", "", rootAuth, "token-1")).toBeNull();
    expect(resolvePostAuthRedirectTarget("/workspace", "", rootAuth, "token-1")).toBeNull();
    expect(resolvePostAuthRedirectTarget("/teams", "?next=%2Fteams", rootAuth, "token-1")).toBeNull();
  });

  it("maps locations into stable app route kinds", () => {
    expect(resolveAppRouteKind(location("/join"), null, null, null)).toBe("join");
    expect(resolveAppRouteKind(location("/admin"), null, null, null)).toBe(
      "admin-auth-required"
    );
    expect(resolveAppRouteKind(location("/admin"), rootAuth, "token-1", null)).toBe("admin");
    expect(
      resolveAppRouteKind(location("/teams/team-1"), null, null, null)
    ).toBe("teams-auth-redirect");
    expect(
      resolveAppRouteKind(location("/teams/team-1"), rootAuth, "token-1", null)
    ).toBe("teams");
    expect(
      resolveAppRouteKind(
        location("/"),
        rootAuth,
        "token-1",
        "/teams?tab=runs#active"
      )
    ).toBe("post-auth-redirect");
    expect(resolveAppRouteKind(location("/"), null, null, null)).toBe("workspace");
    expect(resolveAppRouteKind(location("/workspace"), null, null, null)).toBe("workspace");
    expect(resolveAppRouteKind(location("/workspace/agents/agent-1"), null, null, null)).toBe(
      "workspace"
    );
    expect(resolveAppRouteKind(location("/workspace/teams/team-1"), rootAuth, "token-1", null)).toBe(
      "teams"
    );
  });

  it("does not duplicate history entries when navigating to the current path", () => {
    window.history.replaceState({}, "", "/workspace?lens=nodes&node=node-east");
    const pushStateSpy = vi.spyOn(window.history, "pushState");

    navigateToPath("/workspace?lens=nodes&node=node-east");
    expect(pushStateSpy).not.toHaveBeenCalled();

    navigateToPath("/workspace?lens=nodes&node=node-west");
    expect(pushStateSpy).toHaveBeenCalledOnce();
  });
});
