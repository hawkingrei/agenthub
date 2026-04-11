import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  TeamMemberStatusStrip,
  normalizeTeamMemberLifecycle,
  normalizeTeamMemberWorkStatus,
} from "./team_member_status_strip";

function buildMember(
  patch: Partial<TeamMemberLiveState> = {}
): TeamMemberLiveState {
  return {
    member_id: "agent-1",
    role: "worker",
    agent_name: "agent-1",
    lifecycle_status: "running",
    lifecycle_tone: "active",
    run_status: "working",
    step_status: "working",
    pending_inbox_count: 0,
    current_work: "plan",
    ...patch,
  };
}

function renderHtml(node: React.ReactElement): string {
  return renderToStaticMarkup(<MantineProvider>{node}</MantineProvider>);
}

describe("TeamMemberStatusStrip", () => {
  it("normalizes lifecycle status for top-bar display labels", () => {
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "running" }))).toBe(
      "working"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "working" }))).toBe(
      "working"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "idle" }))).toBe("idle");
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "  IDLE  " }))).toBe(
      "idle"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "stopped" }))).toBe(
      "stopped"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "failed" }))).toBe(
      "stopped"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "exited" }))).toBe(
      "stopped"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "completed" }))).toBe(
      "stopped"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "weird" }))).toBe(
      "unknown"
    );
    expect(normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "   " }))).toBe(
      "unknown"
    );
    expect(
      normalizeTeamMemberLifecycle(buildMember({ lifecycle_status: "running", lifecycle_tone: "missing" }))
    ).toBe("missing");
  });

  it("normalizes work status from step_status first then run_status", () => {
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "working", step_status: "working" }))
    ).toBe("working");
    expect(
      normalizeTeamMemberWorkStatus(
        buildMember({ run_status: "working", step_status: "input_required" })
      )
    ).toBe("pending");
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "working", step_status: "blocked" }))
    ).toBe("blocked");
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "working", step_status: "done" }))
    ).toBe("done");
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "idle", step_status: "-" }))
    ).toBe("idle");
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "-", step_status: "-" }))
    ).toBe("no_run");
    expect(
      normalizeTeamMemberWorkStatus(buildMember({ run_status: "weird", step_status: "-" }))
    ).toBe("unknown");
  });

  it("renders top summary and per-member work/agent badges", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamMemberStatusStrip
          members={[
            buildMember({
              member_id: "leader-1",
              role: "leader",
              lifecycle_status: "running",
              run_status: "working",
              step_status: "working",
            }),
            buildMember({
              member_id: "worker-1",
              lifecycle_status: "idle",
              run_status: "submitted",
              step_status: "input_required",
            }),
            buildMember({
              member_id: "worker-2",
              lifecycle_status: "stopped",
              run_status: "working",
              step_status: "done",
            }),
            buildMember({
              member_id: "worker-3",
              lifecycle_status: "failed",
              run_status: "working",
              step_status: "blocked",
            }),
            buildMember({
              member_id: "worker-4",
              lifecycle_status: "missing",
              lifecycle_tone: "missing",
              agent_name: undefined,
              run_status: "-",
              step_status: "-",
            }),
            buildMember({
              member_id: "worker-5",
              lifecycle_status: "unknown_state",
              run_status: "unknown_state",
              step_status: "-",
            }),
          ]}
        />
      </MantineProvider>
    );
    expect(html).toContain("Member Status");
    expect(html).toContain("working=1");
    expect(html).toContain("idle=1");
    expect(html).toContain("stopped=2");
    expect(html).toContain("missing=1");
    expect(html).toContain("unknown=1");
    expect(html).toContain("leader-1");
    expect(html).toContain("worker-4");
    expect(html).toContain("role");
    expect(html).toContain("leader");
    expect(html).toContain("agent");
    expect(html).toMatch(/>-\s*</);
    expect(html).toContain("work:working");
    expect(html).toContain("work:pending");
    expect(html).toContain("work:done");
    expect(html).toContain("work:blocked");
    expect(html).toContain("work:no_run");
    expect(html).toContain("agent:working");
    expect(html).toContain("agent:missing");
  });

  it("renders empty hint when team has no members", () => {
    const html = renderHtml(<TeamMemberStatusStrip members={[]} />);
    expect(html).toContain("No team members");
    expect(html).toContain("No team members found in current team spec.");
  });

  it("renders current work metadata when available and omits it otherwise", () => {
    const withCurrentWork = renderHtml(
      <TeamMemberStatusStrip members={[buildMember({ current_work: "follow up" })]} />
    );
    const withoutCurrentWork = renderHtml(
      <TeamMemberStatusStrip members={[buildMember({ current_work: "" })]} />
    );
    expect(withCurrentWork).toContain("current");
    expect(withCurrentWork).toContain("follow up");
    expect(withoutCurrentWork).not.toContain("current");
  });
});
