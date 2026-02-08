import { describe, expect, it } from "vitest";
import {
  AGENT_NOT_RUNNING_ERROR,
  isAgentActiveStatus,
  sanitizeAgentError,
  shouldIgnoreAgentWsError,
  shouldOpenAgentSocket,
} from "./agent_ws";

describe("agent status helpers", () => {
  it("treats running and idle as active", () => {
    expect(isAgentActiveStatus("running")).toBe(true);
    expect(isAgentActiveStatus("idle")).toBe(true);
    expect(isAgentActiveStatus("stopped")).toBe(false);
    expect(isAgentActiveStatus(null)).toBe(false);
  });

  it("opens sockets only for active statuses", () => {
    expect(shouldOpenAgentSocket("running")).toBe(true);
    expect(shouldOpenAgentSocket("idle")).toBe(true);
    expect(shouldOpenAgentSocket("failed")).toBe(false);
  });

  it("suppresses not-running errors when inactive", () => {
    expect(sanitizeAgentError(AGENT_NOT_RUNNING_ERROR, "stopped")).toBeNull();
    expect(sanitizeAgentError(AGENT_NOT_RUNNING_ERROR, null)).toBeNull();
    expect(sanitizeAgentError(AGENT_NOT_RUNNING_ERROR, "idle")).toBe(
      AGENT_NOT_RUNNING_ERROR
    );
  });

  it("ignores not-running errors when inactive", () => {
    expect(shouldIgnoreAgentWsError(AGENT_NOT_RUNNING_ERROR, "failed")).toBe(true);
    expect(shouldIgnoreAgentWsError(AGENT_NOT_RUNNING_ERROR, "idle")).toBe(false);
  });
});
