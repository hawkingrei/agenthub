import { describe, expect, it } from "vitest";

import { AuthState } from "./types";
import {
  resolveAppRouteKind,
  resolvePostAuthRedirectTarget,
  resolveTeamRoute,
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
  it("redirects unauthenticated team routes to login", () => {
    expect(shouldRedirectTeamsToLogin("/teams", null, null)).toBe(true);
    expect(shouldRedirectTeamsToLogin("/teams/team-1", rootAuth, "token-1")).toBe(false);
    expect(shouldRedirectTeamsToLogin("/admin", null, null)).toBe(false);
  });

  it("resolves team selector and detail routes", () => {
    expect(resolveTeamRoute("/teams")).toEqual({ mode: "selector", teamId: null });
    expect(resolveTeamRoute("/teams/team-1")).toEqual({
      mode: "detail",
      teamId: "team-1",
    });
    expect(resolveTeamRoute("/teams/team%2F1")).toEqual({
      mode: "detail",
      teamId: "team/1",
    });
    expect(resolveTeamRoute("/agents")).toBeNull();
  });

  it("derives the post-auth redirect target only on the agents root", () => {
    expect(
      resolvePostAuthRedirectTarget(
        "/",
        "?next=%2Fteams%3Ftab%3Druns%23active",
        rootAuth,
        "token-1"
      )
    ).toBe("/teams?tab=runs#active");
    expect(resolvePostAuthRedirectTarget("/", "", rootAuth, "token-1")).toBeNull();
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
    expect(resolveAppRouteKind(location("/"), null, null, null)).toBe("agents");
  });
});
