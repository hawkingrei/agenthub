import { describe, expect, it } from "vitest";
import {
  buildLoginRedirectPath,
  isInvalidTokenMessage,
  normalizePostLoginRedirectTarget,
  resolvePostLoginRedirectTarget,
  shouldRedirectOnAuthError,
} from "./auth_redirect";

describe("auth redirect helpers", () => {
  it("detects invalid token messages", () => {
    expect(isInvalidTokenMessage("invalid token")).toBe(true);
    expect(isInvalidTokenMessage("missing authorization token")).toBe(true);
    expect(isInvalidTokenMessage("unauthorized")).toBe(true);
    expect(isInvalidTokenMessage("root required")).toBe(false);
  });

  it("redirects only for 401 with a token and invalid token message", () => {
    expect(shouldRedirectOnAuthError(401, "t", "invalid token")).toBe(true);
    expect(shouldRedirectOnAuthError(401, null, "invalid token")).toBe(false);
    expect(shouldRedirectOnAuthError(401, "t", "root required")).toBe(false);
    expect(shouldRedirectOnAuthError(403, "t", "invalid token")).toBe(false);
  });

  it("normalizes and resolves post-login redirect targets safely", () => {
    expect(normalizePostLoginRedirectTarget("/teams")).toBe("/teams");
    expect(normalizePostLoginRedirectTarget("/teams?tab=runs#active")).toBe(
      "/teams?tab=runs#active"
    );
    expect(normalizePostLoginRedirectTarget("/")).toBeNull();
    expect(normalizePostLoginRedirectTarget("https://example.com")).toBeNull();
    expect(normalizePostLoginRedirectTarget("//example.com")).toBeNull();
    expect(buildLoginRedirectPath("/teams?tab=runs#active")).toBe(
      "/?next=%2Fteams%3Ftab%3Druns%23active"
    );
    expect(buildLoginRedirectPath(null)).toBe("/");
    expect(resolvePostLoginRedirectTarget("?next=%2Fteams%2Frun-1")).toBe(
      "/teams/run-1"
    );
    expect(resolvePostLoginRedirectTarget("?next=https%3A%2F%2Fevil.example")).toBeNull();
  });
});
