// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { AuthState } from "./types";
import {
  buildTeamDetailPath,
  buildTeamWorkspacePath,
  buildWorkspaceNodePath,
  buildWorkspacePath,
  isAgentsWorkbenchRoute,
  isWorkspaceWorkbenchRoute,
  isTeamMemberRouteTab,
  navigateToPath,
  resolveAppRouteKind,
  resolvePostAuthRedirectTarget,
  resolveTeamMemberRouteTab,
  resolveTeamRoute,
  resolveTeamSelectedTaskId,
  resolveWorkspaceLens,
  resolveWorkspaceAgentRoute,
  resolveWorkspaceNodeId,
  resolveWorkspaceNodeRoute,
  shouldHandleInAppLinkClick,
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
    expect(resolveWorkspaceLens("/workspace", "?lens=channels")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace", "?lens=chat")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace", "?lens=threads")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace", "?lens=nodes")).toBe("nodes");
    expect(resolveWorkspaceLens("/workspace", "?lens=unknown")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace", "")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace/nodes", "?lens=unknown")).toBe("nodes");
    expect(resolveWorkspaceLens("/workspace/teams/team-1", "")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace/teams/team-1", "?lens=search")).toBe("channels");
    expect(resolveWorkspaceLens("/workspace/teams/team-1", "?lens=tasks")).toBe("tasks");
    expect(buildWorkspacePath("agent-1", "channels")).toBe(
      "/workspace/agents/agent-1"
    );
    expect(buildTeamDetailPath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamDetailPath("team/1")).toBe("/workspace/teams/team%2F1");
    expect(buildTeamDetailPath("")).toBe("/workspace/teams");
    expect(buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")).toBe(
      "/workspace/teams/team-1?lens=members&member=worker-1&tab=thread"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, " worker-2 ", "member_console")
    ).toBe("/workspace/teams/team-1?lens=members&member=worker-2&tab=member_console");
    expect(buildTeamWorkspacePath("team-1", "channels", "review", 42)).toBe(
      "/workspace/teams/team-1?channel=review&thread=42"
    );
    expect(buildTeamWorkspacePath("team-1", "search", "review")).toBe(
      "/workspace/teams/team-1?channel=review"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "review", null, null, null, "task-77")).toBe(
      "/workspace/teams/team-1?channel=review&task=task-77"
    );
    expect(
      buildTeamWorkspacePath("team-1", "channels", "review", null, null, null, " task-77 ")
    ).toBe("/workspace/teams/team-1?channel=review&task=task-77");
    expect(
      buildTeamWorkspacePath("team-1", "channels", "review", null, null, null, "   ")
    ).toBe("/workspace/teams/team-1?channel=review");
    expect(buildTeamWorkspacePath("team-1", "channels", "all", 42, null, null, "task-77")).toBe(
      "/workspace/teams/team-1?thread=42&task=task-77"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp", "task-9")
    ).toBe("/workspace/teams/team-1?lens=members&task=task-9&member=worker-1&tab=thread");
    expect(resolveWorkspaceNodeId("?lens=nodes&node=node-east")).toBe("node-east");
    expect(resolveWorkspaceNodeId("?lens=nodes")).toBeNull();
    expect(buildWorkspaceNodePath("node-east")).toBe("/workspace/nodes/node-east");
    expect(buildWorkspaceNodePath()).toBe("/workspace/nodes");
    expect(resolveTeamSelectedTaskId("?task=task-9")).toBe("task-9");
    expect(resolveTeamSelectedTaskId("?task=%20task-9%20")).toBe("task-9");
    expect(resolveTeamSelectedTaskId("")).toBe("");
    expect(resolveTeamMemberRouteTab("?tab=thread")).toBe("agent_acp");
    expect(resolveTeamMemberRouteTab("?tab=member_console")).toBe("member_console");
    expect(resolveTeamMemberRouteTab("?tab=overview")).toBeNull();
    expect(isTeamMemberRouteTab("agent_acp")).toBe(true);
    expect(isTeamMemberRouteTab("overview")).toBe(false);
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
    expect(resolveAppRouteKind(location("/workspace/nodes/node-east"), null, null, null)).toBe(
      "workspace"
    );
    expect(resolveAppRouteKind(location("/workspace/teams/team-1"), rootAuth, "token-1", null)).toBe(
      "teams"
    );
  });

  it("resolves canonical and legacy workspace node routes", () => {
    expect(resolveWorkspaceNodeRoute("/workspace/nodes", "")).toEqual({
      mode: "list",
      nodeId: null,
    });
    expect(resolveWorkspaceNodeRoute("/workspace/nodes/node-east", "")).toEqual({
      mode: "detail",
      nodeId: "node-east",
    });
    expect(resolveWorkspaceNodeRoute("/workspace/nodes/node%2Feast", "")).toEqual({
      mode: "detail",
      nodeId: "node/east",
    });
    expect(resolveWorkspaceNodeRoute("/workspace", "?lens=nodes&node=node-east")).toEqual({
      mode: "detail",
      nodeId: "node-east",
    });
    expect(resolveWorkspaceNodeRoute("/workspace", "?lens=nodes")).toEqual({
      mode: "list",
      nodeId: null,
    });
    expect(resolveWorkspaceNodeRoute("/workspace/agents/agent-1", "")).toBeNull();
  });

  it("treats both agents and nodes pages as workspace workbench routes", () => {
    expect(isWorkspaceWorkbenchRoute("/workspace")).toBe(true);
    expect(isWorkspaceWorkbenchRoute("/workspace/agents/agent-1")).toBe(true);
    expect(isWorkspaceWorkbenchRoute("/workspace/nodes")).toBe(true);
    expect(isWorkspaceWorkbenchRoute("/workspace/nodes/node-east")).toBe(true);
    expect(isWorkspaceWorkbenchRoute("/workspace/teams/team-1")).toBe(false);

    expect(isAgentsWorkbenchRoute("/workspace/agents/agent-1")).toBe(true);
    expect(isAgentsWorkbenchRoute("/workspace/nodes/node-east")).toBe(true);
  });

  it("does not duplicate history entries when navigating to the current path", () => {
    window.history.replaceState({}, "", "/workspace/nodes/node-east");
    const pushStateSpy = vi.spyOn(window.history, "pushState");

    navigateToPath("/workspace/nodes/node-east");
    expect(pushStateSpy).not.toHaveBeenCalled();

    navigateToPath("/workspace/nodes/node-west");
    expect(pushStateSpy).toHaveBeenCalledOnce();
  });

  it("only intercepts plain left-clicks for in-app link navigation", () => {
    expect(
      shouldHandleInAppLinkClick({
        button: 0,
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        defaultPrevented: false,
      })
    ).toBe(true);

    expect(
      shouldHandleInAppLinkClick({
        button: 1,
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        defaultPrevented: false,
      })
    ).toBe(false);

    expect(
      shouldHandleInAppLinkClick({
        button: 0,
        metaKey: true,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        defaultPrevented: false,
      })
    ).toBe(false);

    expect(
      shouldHandleInAppLinkClick({
        button: 0,
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        defaultPrevented: true,
      })
    ).toBe(false);
  });
});
