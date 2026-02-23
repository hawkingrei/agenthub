import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TeamMemberAgentStatus } from "./team/member_helpers";
import {
  TeamMemberStatusStrip,
  normalizeTeamMemberLifecycle,
} from "./team_member_status_strip";

function buildMember(
  patch: Partial<TeamMemberAgentStatus> = {}
): TeamMemberAgentStatus {
  return {
    member_id: "agent-1",
    role: "worker",
    agent_name: "agent-1",
    status: "running",
    missing_agent: false,
    ...patch,
  };
}

describe("TeamMemberStatusStrip", () => {
  it("normalizes lifecycle status for top-bar display labels", () => {
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "running" }))).toBe("working");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "working" }))).toBe("working");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "idle" }))).toBe("idle");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "  IDLE  " }))).toBe("idle");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "stopped" }))).toBe("stopped");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "failed" }))).toBe("stopped");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "exited" }))).toBe("stopped");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "completed" }))).toBe(
      "stopped"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "weird" }))).toBe("unknown");
    expect(normalizeTeamMemberLifecycle(buildMember({ status: "   " }))).toBe("unknown");
    expect(normalizeTeamMemberLifecycle(buildMember({ missing_agent: true }))).toBe("missing");
  });

  it("renders top summary and per-member badges", () => {
    const html = renderToStaticMarkup(
      <TeamMemberStatusStrip
        members={[
          buildMember({ member_id: "leader-1", role: "leader", status: "running" }),
          buildMember({ member_id: "worker-1", status: "idle" }),
          buildMember({ member_id: "worker-2", status: "stopped" }),
          buildMember({ member_id: "worker-3", status: "failed" }),
          buildMember({
            member_id: "worker-4",
            status: "missing",
            missing_agent: true,
            agent_name: undefined,
          }),
        ]}
      />
    );
    expect(html).toContain("Member Status");
    expect(html).toContain("working=1");
    expect(html).toContain("idle=1");
    expect(html).toContain("stopped=2");
    expect(html).toContain("missing=1");
    expect(html).toContain("leader-1");
    expect(html).toContain("worker-4");
    expect(html).toContain("role=leader");
    expect(html).toContain("agent=-");
  });

  it("renders empty hint when team has no members", () => {
    const html = renderToStaticMarkup(<TeamMemberStatusStrip members={[]} />);
    expect(html).toContain("No team members found in current team spec.");
  });
});
