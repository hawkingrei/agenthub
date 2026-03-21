import { describe, expect, it } from "vitest";

import {
  canManageAgentNodes,
  removeAgentNodeRecord,
  replaceAgentNodeRecord,
  resolvePostAuthRedirectTarget,
  resolveTeamRoute,
  shouldRedirectTeamsToLogin,
  upsertAgentNodeRecord,
} from "./app";

describe("team route auth redirect", () => {
  const makeNode = (id: string, name = id) => ({
    id,
    name,
    grpc_target: `${id}.internal:50051`,
    tls_server_name: null,
    default_worktree_root: null,
    is_main: id === "main",
    created_at: 1,
    updated_at: 1,
  });

  it("redirects unauthenticated teams route to login", () => {
    expect(shouldRedirectTeamsToLogin("/teams", null, null)).toBe(true);
    expect(
      shouldRedirectTeamsToLogin("/teams/run-1", null, "token-only")
    ).toBe(true);
    expect(
      shouldRedirectTeamsToLogin("/teams", {
        token: "token-1",
        userId: "user-1",
        username: "root",
        role: "root",
      }, null)
    ).toBe(true);
  });

  it("keeps authenticated teams route in place", () => {
    expect(
      shouldRedirectTeamsToLogin("/teams", {
        token: "token-1",
        userId: "user-1",
        username: "root",
        role: "root",
      }, "token-1")
    ).toBe(false);
  });

  it("does not affect non-team routes", () => {
    expect(shouldRedirectTeamsToLogin("/", null, null)).toBe(false);
    expect(shouldRedirectTeamsToLogin("/admin", null, null)).toBe(false);
    expect(shouldRedirectTeamsToLogin("/teams-workbench", null, null)).toBe(false);
  });

  it("resolves selector and detail team routes", () => {
    expect(resolveTeamRoute("/teams")).toEqual({ mode: "selector", teamId: null });
    expect(resolveTeamRoute("/teams/")).toEqual({ mode: "selector", teamId: null });
    expect(resolveTeamRoute("/teams/team-1")).toEqual({
      mode: "detail",
      teamId: "team-1",
    });
    expect(resolveTeamRoute("/teams/team%2F1")).toEqual({
      mode: "detail",
      teamId: "team/1",
    });
    expect(resolveTeamRoute("/teams-workbench")).toBeNull();
    expect(resolveTeamRoute("/agents")).toBeNull();
  });

  it("returns to the pending route after authentication succeeds", () => {
    const auth = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root" as const,
    };
    expect(resolvePostAuthRedirectTarget("/", "?next=%2Fteams%3Ftab%3Druns%23active", auth, "token-1"))
      .toBe("/teams?tab=runs#active");
    expect(resolvePostAuthRedirectTarget("/", "", auth, "token-1")).toBeNull();
    expect(
      resolvePostAuthRedirectTarget(
        "/teams",
        "?next=%2Fteams",
        auth,
        "token-1"
      )
    ).toBeNull();
    expect(
      resolvePostAuthRedirectTarget(
        "/",
        "?next=%2Fteams",
        null,
        null
      )
    ).toBeNull();
  });

  it("restricts agent-node admin surfaces to root users", () => {
    expect(canManageAgentNodes(null)).toBe(false);
    expect(
      canManageAgentNodes({
        token: "token-1",
        userId: "user-1",
        username: "worker",
        role: "user",
      })
    ).toBe(false);
    expect(
      canManageAgentNodes({
        token: "token-1",
        userId: "user-1",
        username: "root",
        role: "root",
      })
    ).toBe(true);
  });

  it("upserts agent node records without relying on state updaters", () => {
    expect(upsertAgentNodeRecord([makeNode("node-a")], makeNode("node-b"))).toEqual([
      makeNode("node-a"),
      makeNode("node-b"),
    ]);
    expect(
      upsertAgentNodeRecord([makeNode("node-a", "old-a")], makeNode("node-a", "new-a"))
    ).toEqual([makeNode("node-a", "new-a")]);
  });

  it("replaces and removes agent node records deterministically", () => {
    expect(
      replaceAgentNodeRecord(
        [makeNode("node-a", "old-a"), makeNode("node-b")],
        makeNode("node-a", "new-a")
      )
    ).toEqual([makeNode("node-a", "new-a"), makeNode("node-b")]);
    expect(removeAgentNodeRecord([makeNode("node-a"), makeNode("node-b")], "node-a")).toEqual([
      makeNode("node-b"),
    ]);
  });
});
