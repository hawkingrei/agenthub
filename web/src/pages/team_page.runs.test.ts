import { describe, expect, it } from "vitest";
import type {
  AgentRecord,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
} from "../api";
import {
  assignCreatedWorkerToDraft,
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildTeamMemberLiveStates,
  buildMailboxPayloadTemplate,
  createInitialTeamDraftState,
  countUnreadConversationMessages,
  mergeMailboxMessages,
  mergeRunPages,
  mergeTeamRunList,
  normalizeSkillSelection,
  parseTeamSpecMembers,
  resolveConversationMaxMessageId,
  resolveMailboxChatActors,
  resolveTeamMemberAgentControlState,
  toggleSkillSelection,
  selectMailboxConversation,
  selectTeamForgeAgents,
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

function buildMailboxMessage(
  messageId: number,
  fromActorId: string,
  toActorId: string,
  payload: unknown,
  status: TeamActorMessageRecord["status"] = "pending"
): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: fromActorId,
    to_actor_id: toActorId,
    channel: "default",
    transport: "local",
    route: null,
    payload,
    status,
    created_at: 1_700_000_000 + messageId,
    delivered_at: status === "delivered" ? 1_700_000_100 + messageId : null,
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

  it("resolves chat actors from leader and selected member", () => {
    expect(
      resolveMailboxChatActors("leader-agent", ["leader-agent", "worker-agent"], "worker-agent")
    ).toEqual({
      fromActorId: "leader-agent",
      toActorId: "worker-agent",
      inboxActorId: "worker-agent",
    });

    expect(resolveMailboxChatActors("missing", ["worker-agent"], "")).toEqual({
      fromActorId: "worker-agent",
      toActorId: "worker-agent",
      inboxActorId: "worker-agent",
    });
  });

  it("merges mailbox messages with dedupe by message id", () => {
    const merged = mergeMailboxMessages(
      [
        buildMailboxMessage(1, "leader-agent", "worker-agent", {
          type: "chat_message",
          text: "task",
        }),
      ],
      [
        buildMailboxMessage(1, "leader-agent", "worker-agent", {
          type: "chat_message",
          text: "task-updated",
        }),
        buildMailboxMessage(2, "worker-agent", "leader-agent", {
          type: "chat_message",
          text: "done",
        }),
      ]
    );
    expect(merged.map((message) => message.message_id)).toEqual([1, 2]);
    expect(
      (merged[0]?.payload as { text?: string } | undefined)?.text
    ).toBe("task-updated");
  });

  it("selects mailbox conversation in both directions", () => {
    const conversation = selectMailboxConversation(
      [
        buildMailboxMessage(1, "leader-agent", "worker-agent", "a"),
        buildMailboxMessage(2, "worker-agent", "leader-agent", "b"),
        buildMailboxMessage(3, "leader-agent", "worker-2", "c"),
      ],
      "leader-agent",
      "worker-agent"
    );
    expect(conversation.map((message) => message.message_id)).toEqual([1, 2]);
  });

  it("builds chat payload for quick IM send", () => {
    expect(buildMailboxChatPayload("hello")).toEqual({
      type: "chat_message",
      text: "hello",
      source: "team_workbench",
    });
  });

  it("builds stable mailbox conversation key", () => {
    expect(buildMailboxConversationKey("leader-agent", "worker-agent")).toBe(
      "leader-agent::worker-agent"
    );
    expect(buildMailboxConversationKey("worker-agent", "leader-agent")).toBe(
      "leader-agent::worker-agent"
    );
    expect(buildMailboxConversationKey("leader-agent", "")).toBe("");
  });

  it("resolves conversation max message id", () => {
    expect(resolveConversationMaxMessageId([])).toBeNull();
    expect(
      resolveConversationMaxMessageId([
        buildMailboxMessage(3, "a", "b", {}),
        buildMailboxMessage(7, "a", "b", {}),
      ])
    ).toBe(7);
  });

  it("counts unread messages after seen watermark", () => {
    const unread = countUnreadConversationMessages(
      [
        buildMailboxMessage(1, "leader-agent", "worker-agent", {}),
        buildMailboxMessage(2, "worker-agent", "leader-agent", {}),
        buildMailboxMessage(3, "leader-agent", "worker-2", {}),
      ],
      "leader-agent",
      "worker-agent",
      1
    );
    expect(unread).toBe(1);
    expect(
      countUnreadConversationMessages(
        [buildMailboxMessage(2, "leader-agent", "worker-agent", {})],
        "leader-agent",
        "worker-agent",
        2
      )
    ).toBe(0);
  });

  it("counts unread as inbound-only for the active actor side", () => {
    const unread = countUnreadConversationMessages(
      [
        buildMailboxMessage(10, "leader-agent", "worker-agent", {}),
        buildMailboxMessage(11, "worker-agent", "leader-agent", {}),
      ],
      "leader-agent",
      "worker-agent",
      0
    );
    expect(unread).toBe(1);
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

  it("uses fallback member lookup when /api/agents list hides team members", () => {
    const statuses = resolveTeamMemberAgentStatuses(
      {
        members: [
          { member_id: "leader-agent", role: "leader" },
          { member_id: "worker-hidden", role: "worker" },
        ],
      },
      [buildAgent("leader-agent", "running")],
      {
        "worker-hidden": buildAgent("worker-hidden", "created"),
      }
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
        member_id: "worker-hidden",
        role: "worker",
        agent_name: "worker-hidden",
        status: "created",
        missing_agent: false,
      },
    ]);
  });

  it("prefers runtime session status over stale agent catalog status", () => {
    const statuses = resolveTeamMemberAgentStatuses(
      {
        members: [
          { member_id: "leader-agent", role: "leader" },
          { member_id: "worker-agent", role: "worker" },
        ],
      },
      [buildAgent("leader-agent", "stopped"), buildAgent("worker-agent", "stopped")],
      undefined,
      [
        {
          member_id: "leader-agent",
          display_name: "leader-agent",
          role: "leader",
          description: null,
          agent_status: "running",
          session_id: "session-leader",
          session_status: "running",
          card: {
            card_id: "card-leader",
            schema_version: "1",
            description: "leader",
            capability_tags: [],
          },
        },
        {
          member_id: "worker-agent",
          display_name: "worker-agent",
          role: "worker",
          description: null,
          agent_status: "running",
          session_id: "session-worker",
          session_status: "running",
          card: {
            card_id: "card-worker",
            schema_version: "1",
            description: "worker",
            capability_tags: [],
          },
        },
      ]
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
        status: "running",
        missing_agent: false,
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

  it("derives selected Team member lifecycle controls from agent presence and busy state", () => {
    const stoppedAgent = buildAgent("worker-agent", "stopped");
    expect(
      resolveTeamMemberAgentControlState(stoppedAgent, "stopped", null)
    ).toEqual({
      canStart: true,
      canStop: false,
      canDelete: true,
    });

    const runningAgent = buildAgent("worker-agent", "running");
    expect(
      resolveTeamMemberAgentControlState(runningAgent, "working", null)
    ).toEqual({
      canStart: false,
      canStop: true,
      canDelete: true,
    });

    expect(resolveTeamMemberAgentControlState(null, "missing", null)).toEqual({
      canStart: false,
      canStop: false,
      canDelete: false,
    });
    expect(
      resolveTeamMemberAgentControlState(runningAgent, "working", "stop-team-member-agent")
    ).toEqual({
      canStart: false,
      canStop: false,
      canDelete: true,
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
    expect(
      normalizeSkillSelection(
        ["team-deliberation-rules"],
        "",
        ["team-worker-executor"],
        ["agenthub-actor-runtime", "team-worker-executor"]
      )
    ).toEqual([
      "agenthub-actor-runtime",
      "team-worker-executor",
      "team-deliberation-rules",
    ]);
  });

  it("toggles allowed skills while preserving required entries", () => {
    const added = toggleSkillSelection(["agenthub-actor-runtime"], "team-worker-executor");
    expect(added).toEqual(["agenthub-actor-runtime", "team-worker-executor"]);
    const removed = toggleSkillSelection(added, "agenthub-actor-runtime");
    expect(removed).toEqual(["agenthub-actor-runtime", "team-worker-executor"]);
    const unchanged = toggleSkillSelection(removed, "custom-skill");
    expect(unchanged).toEqual(["agenthub-actor-runtime", "team-worker-executor"]);
    const keptRoleSkill = toggleSkillSelection(
      ["agenthub-actor-runtime", "team-leader-orchestrator"],
      "team-leader-orchestrator",
      ["agenthub-actor-runtime", "team-leader-orchestrator"]
    );
    expect(keptRoleSkill).toEqual(["agenthub-actor-runtime", "team-leader-orchestrator"]);
  });

  it("assigns newly created worker agent to first empty slot or appends", () => {
    const filled = assignCreatedWorkerToDraft(
      [
        {
          member_id: "",
          description: "",
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
          description: "",
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

  it("filters forge-selectable members to current team forge session ids", () => {
    const pool = selectTeamForgeAgents(
      [
        buildAgent("agent-a", "running"),
        buildAgent("agent-b", "idle"),
        buildAgent("agent-c", "stopped"),
      ],
      ["agent-c", "missing-agent", "agent-a"]
    );
    expect(pool.map((agent) => agent.id)).toEqual(["agent-c", "agent-a"]);
  });

  it("creates initial team draft with empty forge candidate pool", () => {
    const draft = createInitialTeamDraftState();
    expect(draft.leaderMemberId).toBe("");
    expect(draft.workers).toEqual([]);
    expect(draft.teamForgeAgentIds).toEqual([]);
    expect(draft.useSpecOverride).toBe(false);
    expect(draft.newTeamSpec).toBe("{}");
    expect(draft.leaderSkills).toContain("agenthub-actor-runtime");
  });
});
