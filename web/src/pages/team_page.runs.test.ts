import { describe, expect, it } from "vitest";
import type { AgentRecord, TeamRunEventRecord, TeamRunRecord } from "../api";
import {
  assignCreatedWorkerToDraft,
  buildTeamMemberLiveStates,
  buildMailboxPayloadTemplate,
  mergeRunPages,
  mergeTeamRunList,
  normalizeSkillSelection,
  parseTeamSpecMembers,
  toggleSkillSelection,
  resolveTeamMemberLifecycleTone,
  resolveTeamMemberAgentStatuses,
  resolveRunStatusFilter,
  selectTeamPreviewEvents,
  summarizeTeamMemberAgentStatuses,
} from "./team_page";

function buildRun(
  id: string,
  createdAt: number,
  status: TeamRunRecord["status"] = "submitted"
): TeamRunRecord {
  return {
    id,
    team_id: "team-1",
    context_id: `ctx-${id}`,
    status,
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}

function buildRunEvent(eventId: number): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload: { event_id: eventId },
  };
}

function buildAgent(id: string, status: string): AgentRecord {
  return {
    id,
    name: id,
    workdir: "/tmp",
    command: "codex",
    args: [],
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: false,
    status,
    created_at: 1_700_000_000,
    updated_at: 1_700_000_000,
  };
}

describe("team run list helpers", () => {
  it("maps run status filter to optional API status", () => {
    expect(resolveRunStatusFilter("all")).toBeUndefined();
    expect(resolveRunStatusFilter("working")).toBe("working");
  });

  it("merges paged runs with dedupe and latest payload preference", () => {
    const existing = [
      buildRun("run-1", 100, "submitted"),
      buildRun("run-2", 120, "working"),
    ];
    const incoming = [
      buildRun("run-2", 120, "completed"),
      buildRun("run-3", 110, "submitted"),
    ];
    const merged = mergeRunPages(existing, incoming);
    expect(merged.map((run) => run.id)).toEqual(["run-2", "run-3", "run-1"]);
    expect(merged.find((run) => run.id === "run-2")?.status).toBe("completed");
  });

  it("keeps active run on replace when it is outside current page window", () => {
    const previous = [
      buildRun("run-active", 50, "working"),
      buildRun("run-9", 90, "submitted"),
    ];
    const incoming = [buildRun("run-10", 110, "submitted"), buildRun("run-11", 105, "working")];
    const merged = mergeTeamRunList(previous, incoming, "replace", "run-active");
    expect(merged.map((run) => run.id)).toEqual(["run-10", "run-11", "run-active"]);
    expect(merged.some((run) => run.id === "run-active")).toBe(true);
  });

  it("shows only latest five run records before selecting a member", () => {
    const events = [1, 2, 3, 4, 5, 6, 7].map(buildRunEvent);
    const preview = selectTeamPreviewEvents(events, "");
    expect(preview.map((event) => event.event_id)).toEqual([3, 4, 5, 6, 7]);
  });

  it("shows full run records after selecting a specific member", () => {
    const events = [1, 2, 3, 4, 5, 6, 7].map(buildRunEvent);
    const fullList = selectTeamPreviewEvents(events, "agent-worker-1");
    expect(fullList.map((event) => event.event_id)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it("builds mailbox templates for leader assignment and clarification", () => {
    const assignment = buildMailboxPayloadTemplate("leader_task_assignment") as {
      type: string;
      task: string;
    };
    expect(assignment.type).toBe("leader_task_assignment");
    expect(assignment.task.length).toBeGreaterThan(0);

    const clarification = buildMailboxPayloadTemplate("clarification_request") as {
      type: string;
      choices: string[];
      context: Record<string, unknown>;
    };
    expect(clarification.type).toBe("clarification_request");
    expect(Array.isArray(clarification.choices)).toBe(true);
    expect(clarification.context).toEqual({});
  });

  it("builds mailbox templates for worker status reports", () => {
    const done = buildMailboxPayloadTemplate("worker_done") as {
      type: string;
      status: string;
      evidence: string[];
    };
    expect(done.type).toBe("worker_status");
    expect(done.status).toBe("done");
    expect(done.evidence.length).toBeGreaterThan(0);

    const blocked = buildMailboxPayloadTemplate("worker_blocked") as {
      status: string;
      next_action: string;
    };
    expect(blocked.status).toBe("blocked");
    expect(blocked.next_action.length).toBeGreaterThan(0);
  });

  it("parses team spec members with dedupe and invalid-entry filtering", () => {
    const members = parseTeamSpecMembers({
      members: [
        { member_id: "leader-agent", role: "leader" },
        { member_id: "worker-agent", role: "worker" },
        { member_id: "worker-agent", role: "worker" },
        { member_id: "  " },
        { role: "worker" },
        "invalid",
      ],
    });
    expect(members).toEqual([
      { member_id: "leader-agent", role: "leader" },
      { member_id: "worker-agent", role: "worker" },
    ]);
  });

  it("maps team members to lifecycle statuses and marks missing members", () => {
    const statuses = resolveTeamMemberAgentStatuses(
      {
        members: [
          { member_id: "leader-agent", role: "leader" },
          { member_id: "worker-agent", role: "worker" },
          { member_id: "missing-agent", role: "worker" },
        ],
      },
      [buildAgent("leader-agent", "running"), buildAgent("worker-agent", "stopped")]
    );
    expect(statuses).toEqual([
      {
        member_id: "leader-agent",
        role: "leader",
        agent_name: "leader-agent",
        status: "running",
        missing_agent: false,
      },
      {
        member_id: "worker-agent",
        role: "worker",
        agent_name: "worker-agent",
        status: "stopped",
        missing_agent: false,
      },
      {
        member_id: "missing-agent",
        role: "worker",
        status: "missing",
        missing_agent: true,
      },
    ]);
  });

  it("summarizes active/inactive/missing team member counts", () => {
    const summary = summarizeTeamMemberAgentStatuses([
      {
        member_id: "leader-agent",
        role: "leader",
        status: "running",
        missing_agent: false,
      },
      {
        member_id: "worker-agent-1",
        role: "worker",
        status: "idle",
        missing_agent: false,
      },
      {
        member_id: "worker-agent-2",
        role: "worker",
        status: "stopped",
        missing_agent: false,
      },
      {
        member_id: "worker-agent-3",
        role: "worker",
        status: "missing",
        missing_agent: true,
      },
    ]);
    expect(summary).toEqual({
      active: 2,
      inactive: 1,
      missing: 1,
      total: 4,
    });
  });

  it("maps lifecycle tone to active, inactive, and missing", () => {
    expect(
      resolveTeamMemberLifecycleTone({
        member_id: "leader-agent",
        role: "leader",
        status: "running",
        missing_agent: false,
      })
    ).toBe("active");
    expect(
      resolveTeamMemberLifecycleTone({
        member_id: "worker-agent",
        role: "worker",
        status: "stopped",
        missing_agent: false,
      })
    ).toBe("inactive");
    expect(
      resolveTeamMemberLifecycleTone({
        member_id: "missing-agent",
        role: "worker",
        status: "missing",
        missing_agent: true,
      })
    ).toBe("missing");
  });

  it("builds live states with leader first and snapshot run info", () => {
    const liveStates = buildTeamMemberLiveStates(
      [
        {
          member_id: "worker-agent-2",
          role: "worker",
          agent_name: "worker-agent-2",
          status: "stopped",
          missing_agent: false,
        },
        {
          member_id: "leader-agent",
          role: "leader",
          agent_name: "leader-agent",
          status: "running",
          missing_agent: false,
        },
        {
          member_id: "worker-agent-1",
          role: "worker",
          status: "missing",
          missing_agent: true,
        },
      ],
      [
        {
          member_id: "leader-agent",
          role: "leader",
          model: null,
          prompt: null,
          skills: [],
          pending_inbox_count: 2,
          status: "working",
          latest_step: null,
          session_status: null,
        },
        {
          member_id: "worker-agent-2",
          role: "worker",
          model: null,
          prompt: null,
          skills: [],
          pending_inbox_count: 0,
          status: "submitted",
          latest_step: {
            id: "step-worker-2",
            run_id: "run-1",
            step_key: "worker_2",
            member_id: "worker-agent-2",
            remote_task_id: null,
            status: "working",
            attempt: 1,
            depends_on: [],
            input: {},
            output: {},
            error_text: null,
            started_at: null,
            ended_at: null,
          },
          session_status: null,
        },
      ]
    );
    expect(liveStates.map((member) => member.member_id)).toEqual([
      "leader-agent",
      "worker-agent-1",
      "worker-agent-2",
    ]);
    expect(liveStates[0]?.run_status).toBe("working");
    expect(liveStates[0]?.pending_inbox_count).toBe(2);
    expect(liveStates[1]?.run_status).toBe("-");
    expect(liveStates[1]?.lifecycle_tone).toBe("missing");
    expect(liveStates[1]?.current_work).toBe("No active run context.");
    expect(liveStates[2]?.step_status).toBe("working");
    expect(liveStates[2]?.lifecycle_tone).toBe("inactive");
    expect(liveStates[2]?.current_work).toContain("worker_2");
  });

  it("normalizes selected skills with allowlist and fallback", () => {
    expect(
      normalizeSkillSelection(
        ["agenthub-actor-runtime", "unknown-skill", "team-worker-executor"],
        "",
        ["team-leader-orchestrator"]
      )
    ).toEqual(["agenthub-actor-runtime", "team-worker-executor"]);
    expect(normalizeSkillSelection(["unknown-skill"], "", ["team-worker-executor"])).toEqual([
      "agenthub-actor-runtime",
      "team-worker-executor",
    ]);
    expect(normalizeSkillSelection([], "", ["team-worker-executor"])).toEqual([
      "agenthub-actor-runtime",
      "team-worker-executor",
    ]);
    expect(
      normalizeSkillSelection(
        ["team-worker-executor"],
        "custom-skill-a, custom-skill-b",
        ["team-worker-executor"]
      )
    ).toEqual([
      "agenthub-actor-runtime",
      "team-worker-executor",
      "custom-skill-a",
      "custom-skill-b",
    ]);
  });

  it("toggles allowed skills while ignoring unknown entries", () => {
    const added = toggleSkillSelection(["agenthub-actor-runtime"], "team-worker-executor");
    expect(added).toEqual(["agenthub-actor-runtime", "team-worker-executor"]);
    const removed = toggleSkillSelection(added, "agenthub-actor-runtime");
    expect(removed).toEqual(["team-worker-executor"]);
    const unchanged = toggleSkillSelection(removed, "custom-skill");
    expect(unchanged).toEqual(["team-worker-executor"]);
  });

  it("assigns newly created worker agent to first empty slot or appends", () => {
    const filled = assignCreatedWorkerToDraft(
      [
        {
          member_id: "",
          model: "",
          prompt: "worker prompt",
          skills: ["team-worker-executor"],
          custom_skills: "",
        },
      ],
      "worker-1"
    );
    expect(filled[0]?.member_id).toBe("worker-1");

    const appended = assignCreatedWorkerToDraft(
      [
        {
          member_id: "worker-1",
          model: "",
          prompt: "worker prompt",
          skills: ["team-worker-executor"],
          custom_skills: "",
        },
      ],
      "worker-2"
    );
    expect(appended.map((item) => item.member_id)).toEqual(["worker-1", "worker-2"]);

    const unchanged = assignCreatedWorkerToDraft(appended, "worker-1");
    expect(unchanged).toEqual(appended);
  });
});
