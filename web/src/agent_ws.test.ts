import { describe, expect, it } from "vitest";
import {
  AGENT_NOT_RUNNING_ERROR,
  isAgentActiveStatus,
  isAgentUnexpectedExitStatus,
  sanitizeAgentError,
  shouldShowUnexpectedExitNotice,
  shouldIgnoreAgentWsError,
  shouldOpenAgentSocket,
} from "./agent_ws";

describe("agent status helpers", () => {
  it("treats live runtime states as active", () => {
    expect(isAgentActiveStatus("running")).toBe(true);
    expect(isAgentActiveStatus("idle")).toBe(true);
    expect(isAgentActiveStatus("waiting_permission")).toBe(true);
    expect(isAgentActiveStatus("stale_prompt")).toBe(true);
    expect(isAgentActiveStatus("stopped")).toBe(false);
    expect(isAgentActiveStatus(null)).toBe(false);
  });

  it("opens sockets only for active statuses", () => {
    expect(shouldOpenAgentSocket("running")).toBe(true);
    expect(shouldOpenAgentSocket("idle")).toBe(true);
    expect(shouldOpenAgentSocket("waiting_permission")).toBe(true);
    expect(shouldOpenAgentSocket("failed")).toBe(false);
  });

  it("detects unexpected exit statuses", () => {
    expect(isAgentUnexpectedExitStatus("failed")).toBe(true);
    expect(isAgentUnexpectedExitStatus("exited")).toBe(true);
    expect(isAgentUnexpectedExitStatus("stopped")).toBe(false);
    expect(isAgentUnexpectedExitStatus(null)).toBe(false);
  });

  it("shows unexpected exit notice only for active-to-failed transitions", () => {
    expect(shouldShowUnexpectedExitNotice("running", "failed")).toBe(true);
    expect(shouldShowUnexpectedExitNotice("idle", "exited")).toBe(true);
    expect(shouldShowUnexpectedExitNotice("running", "stopped")).toBe(false);
    expect(shouldShowUnexpectedExitNotice("created", "failed")).toBe(false);
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
