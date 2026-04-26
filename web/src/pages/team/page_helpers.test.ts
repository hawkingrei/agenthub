import { describe, expect, it, vi } from "vitest";
import type {
  AgentEvent,
  AgentRecord,
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
  TeamRuntimeControlResponse,
  TeamRuntimeRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamTaskRecord,
} from "../../api";
import {
  resolveAgentWorkspaceStatusView,
  resolveSelectedAgentWorkspaceLabel,
  resolveSelectedAgentWorkspaceMemberId,
  resolveSelectedConversationTask,
  buildAgentLabel,
  DEFAULT_TEAM_THREAD_TITLE,
  DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND,
  formatTs,
  listTeamWorkspaceTasks,
  mergeConversationMessages,
  pickNextWorkerAgentId,
  resolveTeamRuntimeControlTone,
  resolveTeamPageNotice,
  resolveTeamRuntimeStatus,
  resolveSelectedTeamTask,
  shouldClearSelectedConversationTask,
  shouldClearSelectedTeamMember,
  resolveTaskConversationMemberIds,
  resolveTaskMessageSeenByActors,
  refreshTeamConversationMailboxAfterSend,
  sortTasksByActivity,
  TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT,
  TEAM_MEMBER_EVENT_RETENTION_LIMIT,
  TEAM_RUN_EVENT_RETENTION_LIMIT,
  toPrettyJson,
  updateCachedTeamRuntimeStatus,
  upsertAgentEventList,
  upsertEventList,
  upsertRun,
} from "./page_helpers";
import { mergeMailboxMessages } from "./mailbox_helpers";
import type {
  TeamMemberAgentStatus,
  TeamMemberAgentStatusSummary,
  TeamMemberLiveState,
} from "./member_helpers";

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

function buildRunEvent(
  eventId: number,
  payload: unknown = {}
): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload,
  };
}

function buildAgentEvent(
  eventId: number,
  message: string,
  overrides: Partial<AgentEvent> = {}
): AgentEvent {
  return {
    event_id: eventId,
    agent_id: "agent-1",
    session_id: "session-1",
    seq: String(eventId),
    ts: 1_700_000_000 + eventId,
    stream: "stdout",
    message,
    ...overrides,
  };
}

function buildAgent(overrides: Partial<AgentRecord> = {}): AgentRecord {
  return {
    id: "agent-1",
    name: "Leader Agent",
    workdir: "/tmp",
    command: "agenthub-codex-acp",
    args: [],
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: true,
    status: "running",
    created_at: 1_700_000_000,
    updated_at: 1_700_000_001,
    ...overrides,
  };
}

function buildMailboxMessage(
  messageId: number,
  overrides: Partial<TeamActorMessageRecord> = {}
): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: "leader-agent",
    from_peer_id: "",
    from_actor_kind: "agent",
    to_actor_id: "worker-agent",
    to_peer_id: "",
    to_actor_kind: "agent",
    channel: "default",
    transport: "local",
    route: null,
    payload: {
      type: "chat_message",
      text: "hello",
      task_conversation_id: "conv-1",
      task_message_id: 7,
    },
    status: "delivered",
    created_at: 1_700_000_001,
    delivered_at: 1_700_000_010,
    ...overrides,
  };
}

function buildConversationMessage(
  messageId: number,
  overrides: Partial<TeamConversationMessageRecord> = {}
): TeamConversationMessageRecord {
  return {
    message_id: messageId,
    conversation_id: "conv-1",
    task_id: "task-1",
    from_actor_id: "leader-agent",
    to_actor_id: null,
    route: "group_chat",
    payload: {
      type: "chat_message",
      text: `message-${messageId}`,
    },
    created_at: 1_700_000_000 + messageId,
    ...overrides,
  };
}

function buildTask(
  id: string,
  createdAt: number,
  updatedAt = createdAt,
  overrides: Partial<TeamTaskRecord> = {}
): TeamTaskRecord {
  return {
    id,
    team_id: "team-1",
    title: id,
    status: "open",
    created_by_actor_id: "user",
    context: {},
    created_at: createdAt,
    updated_at: updatedAt,
    ...overrides,
  };
}

function buildMemberSummary(
  overrides: Partial<TeamMemberAgentStatusSummary> = {}
): TeamMemberAgentStatusSummary {
  return {
    active: 0,
    inactive: 0,
    missing: 0,
    total: 0,
    ...overrides,
  };
}

function buildRuntime(
  overrides: Partial<TeamRuntimeRecord> = {}
): TeamRuntimeRecord {
  return {
    team_id: "team-1",
    team_name: "Team One",
    status: "running",
    members: [
      {
        member_id: "leader-agent",
        display_name: "Leader Agent",
        role: "leader",
        description: "lead",
        agent_status: "running",
        session_id: "session-leader",
        session_status: "running",
        card: {
          card_id: "card-leader",
          schema_version: "1",
          description: "lead",
          capability_tags: [],
        },
      },
      {
        member_id: "worker-agent",
        display_name: "Worker Agent",
        role: "worker",
        description: "worker",
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
    ],
    ...overrides,
  };
}

function buildMemberLiveState(
  overrides: Partial<TeamMemberLiveState> = {}
): TeamMemberLiveState {
  return {
    member_id: "leader-agent",
    role: "leader",
    agent_name: "Leader Agent",
    lifecycle_status: "running",
    lifecycle_tone: "active",
    run_status: "working",
    step_status: "working",
    pending_inbox_count: 2,
    current_work: "reviewing worker output",
    ...overrides,
  };
}

function buildMemberStatus(
  overrides: Partial<TeamMemberAgentStatus> = {}
): TeamMemberAgentStatus {
  return {
    member_id: "leader-agent",
    role: "leader",
    agent_name: "Leader Agent",
    status: "stopped",
    missing_agent: false,
    ...overrides,
  };
}

describe("team page helpers", () => {
  it("merges conversation messages while preserving unchanged object identity", () => {
    const original = buildConversationMessage(1);
    const prev = [original, buildConversationMessage(2)];
    const next = [
      buildConversationMessage(1),
      buildConversationMessage(2, {
        route: "to_member",
      }),
      buildConversationMessage(3),
    ];

    const merged = mergeConversationMessages(prev, next);
    expect(merged).toHaveLength(3);
    expect(merged[0]).toBe(original);
    expect(merged[1]).not.toBe(prev[1]);
    expect(merged[2]?.message_id).toBe(3);
  });

  it("reuses immutable conversation messages by id even when payload object identity changes", () => {
    const original = buildConversationMessage(1);
    const prev = [original];

    const merged = mergeConversationMessages(prev, [
      buildConversationMessage(1, {
        payload: { type: "chat_message", text: "updated text that should be ignored" },
      }),
    ]);

    expect(merged).toBe(prev);
    expect(merged[0]).toBe(original);
  });

  it("returns the previous conversation array when refresh payload is unchanged", () => {
    const prev = [buildConversationMessage(1), buildConversationMessage(2)];

    const merged = mergeConversationMessages(prev, [
      buildConversationMessage(1),
      buildConversationMessage(2),
    ]);

    expect(merged).toBe(prev);
  });

  it("trims shared conversation state to the newest recent window", () => {
    const prev = Array.from(
      { length: TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT },
      (_, index) => buildConversationMessage(index + 1)
    );

    const merged = mergeConversationMessages(
      prev,
      Array.from(
        { length: TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT + 5 },
        (_, index) => buildConversationMessage(index + 1)
      )
    );

    expect(merged).toHaveLength(TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT);
    expect(merged[0]?.message_id).toBe(6);
    expect(merged[merged.length - 1]?.message_id).toBe(TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT + 5);
  });

  it("keeps a selected thread while tasks are still loading or detail fallback exists", () => {
    const taskList = [buildTask("task-1", 10)];

    expect(
      shouldClearSelectedConversationTask({
        selectedConversationTaskId: "task-missing",
        sharedConversationTaskId: "shared",
        taskList,
        selectedConversationDetailPresent: false,
        tasksLoading: true,
      })
    ).toBe(false);

    expect(
      shouldClearSelectedConversationTask({
        selectedConversationTaskId: "task-missing",
        sharedConversationTaskId: "shared",
        taskList,
        selectedConversationDetailPresent: true,
        tasksLoading: false,
      })
    ).toBe(false);
  });

  it("clears a non-shared selected thread only after task refresh confirms it disappeared", () => {
    expect(
      shouldClearSelectedConversationTask({
        selectedConversationTaskId: "task-missing",
        sharedConversationTaskId: "shared",
        taskList: [buildTask("task-1", 10), buildTask("task-2", 20)],
        selectedConversationDetailPresent: false,
        tasksLoading: false,
      })
    ).toBe(true);

    expect(
      shouldClearSelectedConversationTask({
        selectedConversationTaskId: "shared",
        sharedConversationTaskId: "shared",
        taskList: [buildTask("task-1", 10)],
        selectedConversationDetailPresent: false,
        tasksLoading: false,
      })
    ).toBe(false);
  });

  it("keeps a selected team member while the loaded member list is still empty", () => {
    expect(
      shouldClearSelectedTeamMember({
        selectedMemberId: "worker-1",
        memberIds: [],
      })
    ).toBe(false);
  });

  it("clears a selected team member after loaded members no longer contain it", () => {
    expect(
      shouldClearSelectedTeamMember({
        selectedMemberId: "worker-1",
        memberIds: ["leader-1", "worker-2"],
      })
    ).toBe(true);
  });

  it("does not fall back to a stale focused member when the route explicitly targets another member", () => {
    expect(
      resolveSelectedAgentWorkspaceMemberId({
        selectedMemberId: "",
        focusedAgentMemberId: "worker-1",
        routeSelectedMemberId: "worker-2",
      })
    ).toBe("");
    expect(
      resolveSelectedAgentWorkspaceMemberId({
        selectedMemberId: "worker-2",
        focusedAgentMemberId: "worker-1",
        routeSelectedMemberId: "worker-2",
      })
    ).toBe("worker-2");
    expect(
      resolveSelectedAgentWorkspaceMemberId({
        selectedMemberId: "",
        focusedAgentMemberId: "worker-1",
        routeSelectedMemberId: "",
      })
    ).toBe("worker-1");
  });

  it("upserts run by id and keeps latest-first sort order", () => {
    const list = [buildRun("run-1", 100), buildRun("run-2", 120)];
    const updated = upsertRun(list, buildRun("run-1", 140, "working"));
    expect(updated.map((run) => run.id)).toEqual(["run-1", "run-2"]);
    expect(updated[0]?.status).toBe("working");
  });

  it("upserts team run events with dedupe and monotonic ordering", () => {
    const replace = upsertEventList(
      [buildRunEvent(1), buildRunEvent(2)],
      [buildRunEvent(2, { text: "updated" }), buildRunEvent(3)],
      "replace"
    );
    expect(replace.map((event) => event.event_id)).toEqual([2, 3]);
    expect(replace[0]?.payload).toEqual({ text: "updated" });

    const prepend = upsertEventList(
      [buildRunEvent(3), buildRunEvent(4)],
      [buildRunEvent(2), buildRunEvent(3, { text: "newer" })],
      "prepend"
    );
    expect(prepend.map((event) => event.event_id)).toEqual([2, 3, 4]);
    expect(prepend.find((event) => event.event_id === 3)?.payload).toEqual({});
  });

  it("caps replace-refreshed team run events to the newest retained window", () => {
    const replace = upsertEventList(
      [],
      Array.from(
        { length: TEAM_RUN_EVENT_RETENTION_LIMIT + 5 },
        (_, index) => buildRunEvent(index + 1)
      ),
      "replace"
    );

    expect(replace).toHaveLength(TEAM_RUN_EVENT_RETENTION_LIMIT);
    expect(replace[0]?.event_id).toBe(6);
    expect(replace[replace.length - 1]?.event_id).toBe(105);
  });

  it("keeps explicit older team run history when prepending older pages", () => {
    const prepend = upsertEventList(
      Array.from({ length: TEAM_RUN_EVENT_RETENTION_LIMIT }, (_, index) =>
        buildRunEvent(index + 101)
      ),
      Array.from({ length: TEAM_RUN_EVENT_RETENTION_LIMIT }, (_, index) =>
        buildRunEvent(index + 1)
      ),
      "prepend"
    );

    expect(prepend).toHaveLength(TEAM_RUN_EVENT_RETENTION_LIMIT * 2);
    expect(prepend[0]?.event_id).toBe(1);
    expect(prepend[prepend.length - 1]?.event_id).toBe(200);
  });

  it("upserts agent events with dedupe and monotonic ordering", () => {
    const merged = upsertAgentEventList(
      [buildAgentEvent(5, "old-5"), buildAgentEvent(7, "old-7")],
      [buildAgentEvent(6, "new-6"), buildAgentEvent(7, "new-7")],
      "prepend"
    );
    expect(merged.map((event) => event.event_id)).toEqual([5, 6, 7]);
    expect(merged.find((event) => event.event_id === 7)?.message).toBe("old-7");
  });

  it("preserves same-session older agent history on replace refresh", () => {
    const refreshed = upsertAgentEventList(
      [
        buildAgentEvent(1, "older-1", { session_id: "session-1" }),
        buildAgentEvent(2, "older-2", { session_id: "session-1" }),
      ],
      [
        buildAgentEvent(3, "latest-3", { session_id: "session-1" }),
        buildAgentEvent(4, "latest-4", { session_id: "session-1" }),
      ],
      "replace",
      "session-1"
    );
    expect(refreshed.map((event) => event.event_id)).toEqual([1, 2, 3, 4]);
  });

  it("keeps replace semantics when no session id is provided", () => {
    const refreshed = upsertAgentEventList(
      [buildAgentEvent(1, "old-1"), buildAgentEvent(2, "old-2")],
      [buildAgentEvent(3, "new-3"), buildAgentEvent(4, "new-4")],
      "replace"
    );
    expect(refreshed.map((event) => event.event_id)).toEqual([3, 4]);
  });

  it("caps replace-refreshed member events to the newest retained window", () => {
    const refreshed = upsertAgentEventList(
      [],
      Array.from(
        { length: TEAM_MEMBER_EVENT_RETENTION_LIMIT + 4 },
        (_, index) => buildAgentEvent(index + 1, `event-${index + 1}`)
      ),
      "replace"
    );

    expect(refreshed).toHaveLength(TEAM_MEMBER_EVENT_RETENTION_LIMIT);
    expect(refreshed[0]?.event_id).toBe(5);
    expect(refreshed[refreshed.length - 1]?.event_id).toBe(304);
  });

  it("builds readable agent labels with model metadata", () => {
    const modelFromArgs = buildAgentLabel(
      buildAgent({ args: ["--model", "gpt-5.1"], command: "gemini" })
    );
    expect(modelFromArgs).toContain("gpt-5.1");
    expect(modelFromArgs).toContain("Leader Agent");
    expect(modelFromArgs).toContain("agent-1");

    const fallback = buildAgentLabel(
      buildAgent({ command: "/usr/local/bin/custom-agent", args: [] })
    );
    expect(fallback).toContain("custom-agent");
  });

  it("picks next available worker agent id and handles exhaustion", () => {
    const agents = [buildAgent({ id: "a1" }), buildAgent({ id: "a2" })];
    expect(pickNextWorkerAgentId(agents, new Set(["a1"]))).toBe("a2");
    expect(pickNextWorkerAgentId(agents, new Set(["a1", "a2"]))).toBe("");
  });

  it("sorts team tasks by recent activity and resolves workspace versus conversation tasks", () => {
    const tasks = [
      buildTask("task-1", 100, 120),
      buildTask("task-2", 110, 110),
      buildTask("task-3", 90, 120, {
        title: DEFAULT_TEAM_THREAD_TITLE,
        context: { bootstrap_kind: DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND },
      }),
    ];
    expect(sortTasksByActivity(tasks).map((task) => task.id)).toEqual([
      "task-1",
      "task-3",
      "task-2",
    ]);
    expect(resolveSelectedTeamTask(tasks, "task-3", "team-1")?.id).toBe("task-1");
    expect(resolveSelectedTeamTask(tasks, "task-2", "team-1")?.id).toBe("task-2");
    expect(resolveSelectedTeamTask(tasks, "missing", "team-1")?.id).toBe("task-1");
    expect(listTeamWorkspaceTasks(tasks, "team-1").map((task) => task.id)).toEqual([
      "task-1",
      "task-2",
    ]);
    expect(resolveSelectedTeamTask([buildTask("task-1", 100, 120)], "", "team-1")?.id).toBe(
      "task-1"
    );
    expect(DEFAULT_TEAM_THREAD_TITLE).toBe("all");
    expect(DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND).toBe("shared_thread");
  });

  it("falls back to the fetched conversation detail task when the visible task list is stale", () => {
    const fallbackTask = buildTask("task-thread", 120, 140, {
      title: "Recovered thread",
      status: "in_progress",
    });

    expect(
      resolveSelectedConversationTask({
        taskList: [],
        selectedTaskId: "task-thread",
        sharedConversation: null,
        fallbackTask,
      })
    ).toEqual(fallbackTask);
  });

  it("resolves seen-by coverage from delivered mailbox fan-out", () => {
    const seen = resolveTaskMessageSeenByActors(
      [
        buildMailboxMessage(1),
        buildMailboxMessage(2, {
          to_actor_id: "worker-agent-2",
          payload: JSON.stringify({
            type: "chat_message",
            text: "hello",
            task_conversation_id: "conv-1",
            task_message_id: 7,
          }),
        }),
        buildMailboxMessage(3, {
          status: "pending",
          to_actor_id: "worker-agent-3",
        }),
        buildMailboxMessage(4, {
          to_actor_id: "worker-agent",
          payload: {
            type: "chat_message",
            text: "other",
            task_conversation_id: "conv-2",
            task_message_id: 9,
          },
        }),
      ],
      "conv-1",
      ["worker-agent", "worker-agent-2", "worker-agent-3"]
    );

    expect(seen).toEqual({
      7: ["worker-agent", "worker-agent-2"],
    });
  });

  it("resolves seen-by coverage from merged visible and shared-thread mailbox sources", () => {
    const seen = resolveTaskMessageSeenByActors(
      mergeMailboxMessages(
        [
          buildMailboxMessage(10, {
            status: "pending",
            to_actor_id: "worker-agent",
            payload: {
              type: "chat_message",
              text: "visible snapshot copy",
              task_conversation_id: "conv-1",
              task_message_id: 42,
            },
          }),
        ],
        [
          buildMailboxMessage(11, {
            status: "delivered",
            to_actor_id: "worker-agent",
            payload: {
              type: "chat_message",
              text: "shared-thread mailbox copy",
              task_conversation_id: "conv-1",
              task_message_id: 42,
            },
          }),
        ]
      ),
      "conv-1",
      ["worker-agent"]
    );

    expect(seen).toEqual({
      42: ["worker-agent"],
    });
  });

  it("prefers team runtime members for shared-thread seen-by resolution", () => {
    expect(
      resolveTaskConversationMemberIds(
        [
          { member_id: "leader-agent" },
          { member_id: "worker-agent" },
        ],
        [{ member_id: "stale-run-member" }]
      )
    ).toEqual(["leader-agent", "worker-agent"]);
    expect(
      resolveTaskConversationMemberIds(null, [{ member_id: "snapshot-only-member" }])
    ).toEqual(["snapshot-only-member"]);
  });

  it("refreshes shared-thread mailbox after send when there is no active run", async () => {
    const refreshSnapshot = vi.fn(async () => undefined);
    const refreshEvents = vi.fn(async () => undefined);
    const refreshTaskMessages = vi.fn(async () => undefined);

    await refreshTeamConversationMailboxAfterSend({
      activeRunId: "",
      taskId: "task-all",
      refreshSnapshot,
      refreshEvents,
      refreshTaskMessages,
    });

    expect(refreshTaskMessages).toHaveBeenCalledWith("task-all");
    expect(refreshSnapshot).not.toHaveBeenCalled();
    expect(refreshEvents).not.toHaveBeenCalled();
  });

  it("treats null active run ids as no-active-run during shared-thread refresh", async () => {
    const refreshSnapshot = vi.fn(async () => undefined);
    const refreshEvents = vi.fn(async () => undefined);
    const refreshTaskMessages = vi.fn(async () => undefined);

    await refreshTeamConversationMailboxAfterSend({
      activeRunId: null,
      taskId: "task-all",
      refreshSnapshot,
      refreshEvents,
      refreshTaskMessages,
    });

    expect(refreshTaskMessages).toHaveBeenCalledWith("task-all");
    expect(refreshSnapshot).not.toHaveBeenCalled();
    expect(refreshEvents).not.toHaveBeenCalled();
  });

  it("refreshes active run snapshot after send when execution is live", async () => {
    const refreshSnapshot = vi.fn(async () => undefined);
    const refreshEvents = vi.fn(async () => undefined);
    const refreshTaskMessages = vi.fn(async () => undefined);

    await refreshTeamConversationMailboxAfterSend({
      activeRunId: "run-123",
      taskId: "task-all",
      refreshSnapshot,
      refreshEvents,
      refreshTaskMessages,
    });

    expect(refreshSnapshot).toHaveBeenCalledWith("run-123");
    expect(refreshEvents).toHaveBeenCalledWith("run-123");
    expect(refreshTaskMessages).not.toHaveBeenCalled();
  });

  it("resolves team runtime status from member availability summary", () => {
    expect(resolveTeamRuntimeStatus(buildMemberSummary())).toMatchObject({
      status: "stopped",
      label: "team stopped",
      tone: "inactive",
    });
    expect(
      resolveTeamRuntimeStatus(
        buildMemberSummary({ active: 3, inactive: 0, missing: 0, total: 3 })
      )
    ).toMatchObject({
      status: "running",
      label: "team running",
      tone: "active",
    });
    expect(
      resolveTeamRuntimeStatus(
        buildMemberSummary({ active: 2, inactive: 1, missing: 0, total: 3 })
      )
    ).toMatchObject({
      status: "degraded",
      label: "team degraded",
      tone: "warning",
    });
  });

  it("prefers explicit backend team runtime status when present", () => {
    expect(
      resolveTeamRuntimeStatus(buildMemberSummary({ active: 0, inactive: 3, missing: 0, total: 3 }), {
        status: "running",
        members: [
          { member_id: "leader", session_id: "session-leader" },
          { member_id: "worker-1", session_id: "session-worker-1" },
          { member_id: "worker-2", session_id: "session-worker-2" },
        ],
      })
    ).toMatchObject({
      status: "running",
      label: "team running",
      tone: "active",
      online: 3,
      total: 3,
    });
  });

  it("maps team runtime status to mantine control tones", () => {
    expect(resolveTeamRuntimeControlTone("running")).toEqual({
      statusColor: "teal",
      countColor: "teal",
    });
    expect(resolveTeamRuntimeControlTone("degraded")).toEqual({
      statusColor: "yellow",
      countColor: "yellow",
    });
    expect(resolveTeamRuntimeControlTone("stopped")).toEqual({
      statusColor: "gray",
      countColor: "gray",
    });
  });

  it("classifies runtime summaries separately from actual warnings", () => {
    expect(resolveTeamPageNotice("Team runtime updated (started=3)")).toEqual({
      kind: "runtime",
      title: "Team runtime",
      message: "Team runtime updated (started=3)",
    });
    expect(resolveTeamPageNotice("Team runtime stopped (stopped=3)")).toEqual({
      kind: "runtime",
      title: "Team runtime",
      message: "Team runtime stopped (stopped=3)",
    });
    expect(resolveTeamPageNotice("Unable to initialize shared team thread.")).toEqual({
      kind: "warning",
      title: "Team runtime update",
      message: "Unable to initialize shared team thread.",
    });
    expect(resolveTeamPageNotice("   ")).toBeNull();
  });

  it("clears cached runtime session ids when stop-team optimistic update applies", () => {
    const control: TeamRuntimeControlResponse["members"] = [
      { member_id: "leader-agent", session_id: "session-leader", action: "stopped" },
      { member_id: "worker-agent", session_id: "session-worker", action: "stopped" },
    ];
    const updated = updateCachedTeamRuntimeStatus(
      buildRuntime(),
      "team-1",
      "Team One",
      "stopped",
      control,
      null
    );
    expect(updated?.status).toBe("stopped");
    expect(updated?.members.every((member) => member.session_id == null)).toBe(true);
    expect(updated?.members.every((member) => member.session_status === "stopped")).toBe(true);
  });

  it("synthesizes optimistic runtime members when start-team has no cached runtime yet", () => {
    const control: TeamRuntimeControlResponse["members"] = [
      { member_id: "leader-agent", session_id: "session-leader", action: "started" },
      { member_id: "worker-agent", session_id: "session-worker", action: "started" },
    ];
    const updated = updateCachedTeamRuntimeStatus(
      undefined,
      "team-1",
      "Team One",
      "running",
      control,
      () => "running",
      [
        buildMemberStatus(),
        buildMemberStatus({
          member_id: "worker-agent",
          role: "worker",
          agent_name: "Worker Agent",
        }),
      ]
    );
    expect(updated?.status).toBe("running");
    expect(updated?.members).toHaveLength(2);
    expect(updated?.members.map((member) => member.member_id)).toEqual([
      "leader-agent",
      "worker-agent",
    ]);
    expect(updated?.members.every((member) => member.session_status === "running")).toBe(true);
    expect(updated?.members.every((member) => member.agent_status === "running")).toBe(true);
    expect(updated?.members.map((member) => member.session_id)).toEqual([
      "session-leader",
      "session-worker",
    ]);
  });

  it("formats timestamps and pretty prints JSON safely", () => {
    expect(formatTs(null)).toBe("-");
    expect(formatTs(0)).toBe("-");
    expect(formatTs(1_700_000_000)).toBe(new Date(1_700_000_000 * 1000).toLocaleString());

    expect(toPrettyJson({ a: 1 })).toBe('{\n  "a": 1\n}');
    const circular: { self?: unknown } = {};
    circular.self = circular;
    expect(toPrettyJson(circular)).toBe("[object Object]");
  });

  it("prefers agent display names when resolving focused agent labels", () => {
    expect(
      resolveSelectedAgentWorkspaceLabel(
        "leader-agent",
        buildMemberLiveState({ agent_name: "Leader Agent" }),
        "Fallback Agent"
      )
    ).toBe("Leader Agent");
    expect(
      resolveSelectedAgentWorkspaceLabel("leader-agent", null, "Fallback Agent")
    ).toBe("Fallback Agent");
    expect(resolveSelectedAgentWorkspaceLabel("leader-agent", null, null)).toBe("leader-agent");
    expect(resolveSelectedAgentWorkspaceLabel("   ", null, null)).toBe("Agent");
  });

  it("summarizes focused agent lifecycle, work, inbox, and current work", () => {
    expect(resolveAgentWorkspaceStatusView(buildMemberLiveState())).toEqual({
      role: "leader",
      lifecycle: "working",
      work: "working",
      inbox: "2",
      currentWork: "reviewing worker output",
    });
    expect(
      resolveAgentWorkspaceStatusView(
        buildMemberLiveState({
          role: "worker",
          lifecycle_status: "stopped",
          run_status: "-",
          step_status: "-",
          pending_inbox_count: null,
          current_work: "   ",
        })
      )
    ).toEqual({
      role: "worker",
      lifecycle: "stopped",
      work: "no run",
      inbox: "-",
      currentWork: "No direct activity reported yet.",
    });
  });
});
