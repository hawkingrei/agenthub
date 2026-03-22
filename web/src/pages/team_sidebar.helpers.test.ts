import { describe, expect, it } from "vitest";

import { TeamMemberLiveState } from "./team/member_helpers";
import {
  formatTeamMemberSummary,
  formatWorkLabel,
  resolveMemberIndicatorClassName,
  resolveMemberPrimaryLabel,
} from "./team_sidebar";

function buildMember(overrides: Partial<TeamMemberLiveState> = {}): TeamMemberLiveState {
  return {
    member_id: "worker-1",
    role: "worker",
    agent_id: "agent-1",
    agent_name: "Worker One",
    lifecycle: "working",
    session_status: "active",
    latest_step_id: null,
    latest_step_key: null,
    current_work: null,
    current_run_id: null,
    pending_inbox_count: 0,
    ...overrides,
  };
}

describe("team_sidebar helpers", () => {
  it("formats work labels and member summaries for sidebar metadata", () => {
    expect(formatWorkLabel("no_run")).toBe("idle");
    expect(formatWorkLabel("done")).toBe("done");

    expect(
      formatTeamMemberSummary({
        total: 4,
        active: 2,
        inactive: 1,
        missing: 1,
      })
    ).toBe("4 members · 2 active · 1 idle · 1 missing");
    expect(
      formatTeamMemberSummary({
        total: 2,
        active: 2,
        inactive: 0,
        missing: 0,
      })
    ).toBe("2 members · 2 active");
    expect(formatTeamMemberSummary()).toBeNull();
  });

  it("prefers trimmed agent names and falls back to member ids", () => {
    expect(resolveMemberPrimaryLabel(buildMember())).toBe("Worker One");
    expect(resolveMemberPrimaryLabel(buildMember({ agent_name: "  " }))).toBe("worker-1");
  });

  it("maps lifecycle/work combinations to the expected indicator classes", () => {
    expect(resolveMemberIndicatorClassName("missing", "working")).toBe("bg-rose-500");
    expect(resolveMemberIndicatorClassName("working", "pending")).toBe("bg-emerald-500");
    expect(resolveMemberIndicatorClassName("stopped", "done")).toBe("bg-emerald-400");
    expect(resolveMemberIndicatorClassName("stopped", "idle")).toBe("bg-slate-400");
    expect(resolveMemberIndicatorClassName("idle", "idle")).toBe("bg-slate-300");
  });
});
