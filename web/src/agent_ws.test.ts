import { describe, expect, it } from "vitest";
import {
  AGENT_NOT_RUNNING_ERROR,
  sanitizeAgentError,
  shouldIgnoreAgentWsError,
  shouldOpenAgentSocket,
} from "./agent_ws";

describe("agent ws helpers", () => {
  it("opens sockets only when running", () => {
    expect(shouldOpenAgentSocket("running")).toBe(true);
    expect(shouldOpenAgentSocket("stopped")).toBe(false);
    expect(shouldOpenAgentSocket(null)).toBe(false);
  });

  it("ignores not-running errors when agent is not running", () => {
    expect(shouldIgnoreAgentWsError(AGENT_NOT_RUNNING_ERROR, "stopped")).toBe(
      true
    );
    expect(shouldIgnoreAgentWsError(AGENT_NOT_RUNNING_ERROR, "running")).toBe(
      false
    );
    expect(shouldIgnoreAgentWsError("other error", "stopped")).toBe(false);
  });

  it("clears stale not-running errors on non-running status", () => {
    expect(sanitizeAgentError(AGENT_NOT_RUNNING_ERROR, "stopped")).toBe(null);
    expect(sanitizeAgentError(AGENT_NOT_RUNNING_ERROR, "running")).toBe(
      AGENT_NOT_RUNNING_ERROR
    );
    expect(sanitizeAgentError("other error", "stopped")).toBe("other error");
  });
});
