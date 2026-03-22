import { describe, expect, it } from "vitest";

import {
  buildTeamDetailPath,
  formatTeamRuntimeActionSummary,
  parseTeamAgentInputSessionMismatch,
  validateRunInputJson,
} from "./team_page";

describe("team_page helpers", () => {
  it("parses agent session mismatch payloads and rejects malformed messages", () => {
    expect(
      parseTeamAgentInputSessionMismatch(
        "agent session mismatch: expected=session-a running=session-b"
      )
    ).toEqual({
      expected: "session-a",
      running: "session-b",
    });
    expect(parseTeamAgentInputSessionMismatch("other error")).toBeNull();
    expect(
      parseTeamAgentInputSessionMismatch("agent session mismatch: expected= running=session-b")
    ).toBeNull();
  });

  it("builds team detail paths with escaped identifiers", () => {
    expect(buildTeamDetailPath("team-1")).toBe("/teams/team-1");
    expect(buildTeamDetailPath("team/1")).toBe("/teams/team%2F1");
  });

  it("summarizes runtime actions by grouped member operation", () => {
    expect(
      formatTeamRuntimeActionSummary("start", [
        { action: "created" },
        { action: "reused" },
        { action: "created" },
      ])
    ).toBe("Team runtime updated (created=2, reused=1)");
    expect(formatTeamRuntimeActionSummary("stop", [])).toBe("Team runtime stopped");
  });

  it("validates optional run input JSON", () => {
    expect(validateRunInputJson("   ")).toEqual({
      parsed: undefined,
      error: null,
    });
    expect(validateRunInputJson('{"task":"sync","count":2}')).toEqual({
      parsed: { task: "sync", count: 2 },
      error: null,
    });
    expect(validateRunInputJson("{invalid")).toEqual({
      parsed: undefined,
      error: expect.stringContaining("Run input must be valid JSON"),
    });
  });
});
