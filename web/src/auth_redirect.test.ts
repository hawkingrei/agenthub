import { afterEach, describe, expect, it, vi } from "vitest";
import {
  buildLoginRedirectPath,
  clearAuthAndRedirect,
  isInvalidTokenMessage,
  normalizePostLoginRedirectTarget,
  resolvePostLoginRedirectTarget,
  shouldRedirectOnAuthError,
} from "./auth_redirect";

const originalLocationDescriptor = Object.getOwnPropertyDescriptor(globalThis, "location");

function installLocation(pathname: string, search = "", hash = "") {
  const location = {
    pathname,
    search,
    hash,
    href: `${pathname}${search}${hash}`,
    reload: vi.fn(),
  };
  Object.defineProperty(globalThis, "location", {
    configurable: true,
    value: location,
  });
  return location;
}

function restoreLocation() {
  if (originalLocationDescriptor) {
    Object.defineProperty(globalThis, "location", originalLocationDescriptor);
  } else {
    Reflect.deleteProperty(globalThis, "location");
  }
}

describe("auth redirect helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    restoreLocation();
  });

  it("detects invalid token messages", () => {
    expect(isInvalidTokenMessage(null)).toBe(false);
    expect(isInvalidTokenMessage("invalid token")).toBe(true);
    expect(isInvalidTokenMessage(" Invalid Token ")).toBe(true);
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
    expect(normalizePostLoginRedirectTarget(undefined)).toBeNull();
    expect(normalizePostLoginRedirectTarget("   ")).toBeNull();
    expect(normalizePostLoginRedirectTarget("/")).toBeNull();
    expect(normalizePostLoginRedirectTarget("teams")).toBeNull();
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

  it("reloads instead of rewriting the same login redirect", () => {
    const location = installLocation("/", "?next=%2Fteams", "");

    clearAuthAndRedirect("/teams");

    expect(location.reload).toHaveBeenCalledOnce();
    expect(location.href).toBe("/?next=%2Fteams");
  });

  it("updates href when redirect target differs from the current page", () => {
    const location = installLocation("/workspace", "", "");

    clearAuthAndRedirect("/teams");

    expect(location.reload).not.toHaveBeenCalled();
    expect(location.href).toBe("/?next=%2Fteams");
  });
});
