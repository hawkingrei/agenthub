import { describe, expect, it } from "vitest";
import {
  shouldAttemptPermissionJump,
  type PendingPermissionJumpState,
} from "./components/use_agents_permission_jump";

describe("shouldAttemptPermissionJump", () => {
  const pending: PendingPermissionJumpState = {
    toolCallId: "tool-1",
    sessionId: "session-1",
    attempts: 0,
  };

  it("stays idle without a pending jump", () => {
    expect(shouldAttemptPermissionJump(null, "conversation", null)).toBe("idle");
  });

  it("waits outside the conversation tab", () => {
    expect(shouldAttemptPermissionJump(pending, "debug", "session-1")).toBe("wait");
  });

  it("waits until the matching session is active", () => {
    expect(shouldAttemptPermissionJump(pending, "conversation", "session-2")).toBe("wait");
  });

  it("clears once the retry budget is exhausted", () => {
    expect(
      shouldAttemptPermissionJump(
        { ...pending, attempts: 24 },
        "conversation",
        "session-1"
      )
    ).toBe("clear");
  });

  it("attempts when the conversation tab and session match", () => {
    expect(shouldAttemptPermissionJump(pending, "conversation", "session-1")).toBe(
      "attempt"
    );
  });
});
