import { describe, expect, it } from "vitest";
import {
  isInvalidTokenMessage,
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
});
