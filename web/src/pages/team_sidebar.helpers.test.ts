import { describe, expect, it } from "vitest";

import { TeamMemberLiveState } from "./team/member_helpers";
import {
  formatWorkLabel,
  resolveMemberIndicatorClassName,
  resolveMemberPrimaryLabel,
} from "./team_sidebar";

function buildMember(overrides: Partial<TeamMemberLiveState> = {}): TeamMemberLiveState {
  return {
    member_id: "worker-1",
    role: "worker",
    agent_name: "Worker One",
    lifecycle_status: "working",
    lifecycle_tone: "active",
    run_status: "working",
    step_status: "idle",
    current_work: "",
    pending_inbox_count: 0,
    ...overrides,
  };
}

describe("team_sidebar helpers", () => {
  it("formats work labels for sidebar metadata", () => {
    expect(formatWorkLabel("no_run")).toBe("idle");
    expect(formatWorkLabel("done")).toBe("done");
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
