import { describe, expect, it } from "vitest";

import { resolvePostAuthRedirectTarget, shouldRedirectTeamsToLogin } from "./app";

describe("team route auth redirect", () => {
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
});
