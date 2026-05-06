export type StoredAuthState = {
  token: string;
  userId: string;
  username: string;
  role: string;
};

export type E2eAgentRecord = {
  id: string;
  name: string;
  workdir: string;
  command: string;
  args: string[];
  target_node_id?: string | null;
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  status: string;
  created_at: number;
  updated_at: number;
};

export type E2eAgentNodeRecord = {
  id: string;
  name: string;
  grpc_target?: string | null;
  tls_server_name?: string | null;
  default_worktree_root?: string | null;
  last_seen_at?: number | null;
  is_main: boolean;
  created_at: number;
  updated_at: number;
};

export type E2eAgentNodeJoinBootstrapInfo = {
  enabled: boolean;
  bootstrap_token?: string | null;
  grpc_listen_addr?: string | null;
  security_mode?: string | null;
  cert_dir?: string | null;
  issuer?: string | null;
  audience?: string | null;
};

export type TeamSpecMember = {
  member_id: string;
  role?: string;
  description?: string;
  model?: string;
  skills?: string[];
};

export type TeamSpecStep = {
  step_key: string;
  member_id?: string;
  depends_on?: string[];
};

export type TeamSpecPayload = {
  spec_version?: number;
  entrypoint?: string;
  coordinator_member_id?: string;
  members: TeamSpecMember[];
  steps?: TeamSpecStep[];
};

export type CreateTeamPayload = {
  name: string;
  description?: string;
  spec: TeamSpecPayload;
};

export type UpdateTeamSpecPayload = {
  spec: TeamSpecPayload;
  expected_updated_at: number;
};

export type TeamDefinitionRecord = {
  id: string;
  name: string;
  description?: string | null;
  spec: TeamSpecPayload;
  created_at: number;
  updated_at: number;
};

export type TeamRunRecord = {
  id: string;
  team_id: string;
  context_id: string;
  status:
    | "submitted"
    | "working"
    | "input_required"
    | "completed"
    | "failed"
    | "canceled";
  input: Record<string, unknown>;
  created_at: number;
  started_at: number | null;
  ended_at: number | null;
};

export type TeamRuntimeStatus = "running" | "stopped" | "degraded";

export type MockTeamRuntimeState = {
  status: TeamRuntimeStatus;
};

export type TeamTaskRecord = {
  id: string;
  team_id: string;
  title: string;
  status: "open" | "in_progress" | "completed" | "archived";
  created_by_actor_id: string;
  context: Record<string, unknown>;
  created_at: number;
  updated_at: number;
};

export type TeamConversationMessageRecord = {
  message_id: number;
  conversation_id: string;
  task_id: string;
  from_actor_id: string;
  to_actor_id: string | null;
  route: "to_coordinator" | "to_member" | "group_chat";
  payload: unknown;
  created_at: number;
};

export type TeamActorMessageRecord = {
  message_id: number;
  run_id: string;
  from_actor_id: string;
  to_actor_id: string;
  channel: string;
  transport: "local" | "remote";
  route: Record<string, unknown> | null;
  payload: unknown;
  status: "pending" | "delivered" | "dead_letter";
  created_at: number;
  delivered_at: number | null;
};

export type TeamPageFixture = {
  now: number;
  auth: StoredAuthState;
  agents: E2eAgentRecord[];
  nodes: E2eAgentNodeRecord[];
  nodeJoinBootstrap: E2eAgentNodeJoinBootstrapInfo;
  teams: TeamDefinitionRecord[];
  getCreatePayload: () => CreateTeamPayload | null;
  getUpdateSpecPayloads: () => Array<{ teamId: string; payload: UpdateTeamSpecPayload }>;
};

export function jsonResponse(data: unknown, status = 200): {
  status: number;
  contentType: string;
  body: string;
} {
  return {
    status,
    contentType: "application/json",
    body: JSON.stringify(data),
  };
}

export function buildTeamRun(
  teamId: string,
  status: TeamRunRecord["status"],
  createdAt: number,
  index: number
): TeamRunRecord {
  return {
    id: `${teamId}-${status}-${index}`,
    team_id: teamId,
    context_id: `ctx-${teamId}-${index}`,
    status,
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}


export async function mockTeamPageApis(
  page: import("@playwright/test").Page
): Promise<TeamPageFixture> {
  const now = 1_700_000_000;
  const auth: StoredAuthState = {
    token: "token-e2e",
    userId: "user-e2e",
    username: "e2e-user",
    role: "root",
  };
  const agents: E2eAgentRecord[] = [
    {
      id: "agent-coordinator-1",
      name: "Coordinator Agent",
      workdir: "/workspace/coordinator",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
    {
      id: "agent-worker-1",
      name: "Worker Agent",
      workdir: "/workspace/worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
    {
      id: "agent-worker-2",
      name: "Worker Agent Two",
      workdir: "/workspace/worker-two",
      command: "kimi",
      args: ["acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
  ];
  const nodes: E2eAgentNodeRecord[] = [
    {
      id: "main",
      name: "Main Node",
      grpc_target: null,
      tls_server_name: null,
      default_worktree_root: null,
      last_seen_at: now,
      is_main: true,
      created_at: now,
      updated_at: now,
    },
  ];
  const nodeJoinBootstrap: E2eAgentNodeJoinBootstrapInfo = {
    enabled: true,
    bootstrap_token: "bootstrap-token-e2e",
    grpc_listen_addr: "0.0.0.0:50051",
    security_mode: "tls",
    cert_dir: "/etc/agenthub/internal-grpc",
    issuer: "agenthub",
    audience: "agenthub-internal",
  };
  const teams: TeamDefinitionRecord[] = [];
  const teamRuntimeStateById = new Map<string, MockTeamRuntimeState>();
  const tasksByTeamId = new Map<string, TeamTaskRecord[]>();
  const taskMessagesById = new Map<string, TeamConversationMessageRecord[]>();
  const taskCounterByTeamId = new Map<string, number>();
  const mailboxMessagesByRunId = new Map<string, TeamActorMessageRecord[]>();
  const runEventCounterByRunId = new Map<string, number>();
  let createTeamPayload: CreateTeamPayload | null = null;
  const updateSpecPayloads: Array<{ teamId: string; payload: UpdateTeamSpecPayload }> = [];

  const ensureTasks = (teamId: string): TeamTaskRecord[] => {
    const existing = tasksByTeamId.get(teamId);
    if (existing) {
      return existing;
    }
    const defaultTask: TeamTaskRecord = {
      id: `task-${teamId}-1`,
      team_id: teamId,
      title: "Default planning conversation",
      status: "open",
      created_by_actor_id: `user:${auth.userId}`,
      context: { source: "e2e-default" },
      created_at: now + 1,
      updated_at: now + 1,
    };
    tasksByTeamId.set(teamId, [defaultTask]);
    taskCounterByTeamId.set(teamId, 1);
    taskMessagesById.set(defaultTask.id, []);
    return [defaultTask];
  };

  const inferRunStatusFromRunId = (runId: string): TeamRunRecord["status"] => {
    const matched = runId.match(/-(submitted|working|input_required|completed|failed|canceled)-/);
    if (!matched) {
      return "working";
    }
    return matched[1] as TeamRunRecord["status"];
  };

  const inferTeamIdFromRunId = (runId: string): string => {
    const matchedTeam = teams.find((team) => runId.startsWith(`${team.id}-`));
    if (matchedTeam) {
      return matchedTeam.id;
    }
    const separatorIndex = runId.indexOf("-");
    if (separatorIndex > 0) {
      return runId.slice(0, separatorIndex);
    }
    return teams[0]?.id ?? "team-e2e";
  };

  const ensureMailboxMessages = (runId: string): TeamActorMessageRecord[] => {
    const existing = mailboxMessagesByRunId.get(runId);
    if (existing) {
      return existing;
    }
    const initial: TeamActorMessageRecord[] = [];
    mailboxMessagesByRunId.set(runId, initial);
    return initial;
  };

  const buildSyntheticRun = (runId: string): TeamRunRecord => {
    const teamId = inferTeamIdFromRunId(runId);
    const status = inferRunStatusFromRunId(runId);
    return {
      id: runId,
      team_id: teamId,
      context_id: `ctx-${runId}`,
      status,
      input: {},
      created_at: now + 300,
      started_at: status === "submitted" ? null : now + 301,
      ended_at:
        status === "completed" || status === "failed" || status === "canceled"
          ? now + 360
          : null,
    };
  };

  const buildSyntheticSnapshot = (run: TeamRunRecord): TeamRunSnapshotRecord => {
    const team =
      teams.find((item) => item.id === run.team_id) ?? {
        id: run.team_id,
        name: run.team_id,
        description: null,
        spec: {
          coordinator_member_id: "agent-coordinator-1",
          members: [{ member_id: "agent-coordinator-1", role: "coordinator", model: "codex" }],
          steps: [{ step_key: "coordinator_plan" }],
        },
        created_at: now,
        updated_at: now,
      };
    const teamMembers = Array.isArray(team.spec.members) ? team.spec.members : [];
    const coordinatorMemberId = team.spec.coordinator_member_id ?? teamMembers[0]?.member_id ?? "agent-coordinator-1";
    const members =
      teamMembers.length > 0
        ? teamMembers.map((member) => {
            const matchedAgent = agents.find((agent) => agent.id === member.member_id);
            const isCoordinator = member.member_id === coordinatorMemberId;
            return {
              member_id: member.member_id,
              role: member.role ?? (isCoordinator ? "coordinator" : "worker"),
              model: member.model ?? null,
              description: member.description ?? null,
              prompt: null,
              skills: [],
              pending_inbox_count: 0,
              status: isCoordinator ? run.status : "submitted",
              latest_step: null,
              session_status: matchedAgent?.status ?? "idle",
            };
          })
        : [
            {
              member_id: coordinatorMemberId,
              role: "coordinator",
              model: "codex",
              description: null,
              prompt: null,
              skills: [],
              pending_inbox_count: 0,
              status: run.status,
              latest_step: null,
              session_status: "idle",
            },
          ];
    const recentMessages = ensureMailboxMessages(run.id).slice(-20);
    return {
      run,
      team,
      coordinator_member_id: coordinatorMemberId,
      members,
      steps: [],
      latest_events: [],
      mailbox: {
        pending: 0,
        delivered: 0,
        dead_letter: 0,
        recent_messages: recentMessages,
      },
    };
  };

  const buildTeamRuntime = (team: TeamDefinitionRecord) => {
    const override = teamRuntimeStateById.get(team.id);
    const members = team.spec.members.map((member) => {
      const agent = agents.find((item) => item.id === member.member_id);
      const agentStatus = override
        ? override.status === "running"
          ? "running"
          : "stopped"
        : agent?.status ?? "created";
      const isRunning = agentStatus === "running";
      const sessionId = isRunning ? `session-${team.id}-${member.member_id}` : null;
      return {
        member_id: member.member_id,
        display_name: agent?.name ?? member.member_id,
        role: member.role ?? "worker",
        description: member.description ?? null,
        agent_status: agentStatus,
        session_id: sessionId,
        session_status: isRunning ? "running" : agentStatus === "stopped" ? "stopped" : null,
        card: {
          card_id: `agenthub://agents/${member.member_id}`,
          schema_version: "agenthub.a2a.discovery_card.v1",
          description: member.description ?? `${member.member_id} runtime`,
          capability_tags: ["team_mailbox_v1"],
        },
      };
    });
    const onlineCount = members.filter((member) => member.session_id).length;
    const status: TeamRuntimeStatus = override?.status ??
      (onlineCount === 0
        ? "stopped"
        : onlineCount === members.length
          ? "running"
          : "degraded");
    return {
      team_id: team.id,
      team_name: team.name,
      status,
      members,
    };
  };

  await page.addInitScript((storedAuth: StoredAuthState) => {
    window.localStorage.setItem("agenthub_auth", JSON.stringify(storedAuth));
  }, auth);

  await page.route("**/api/auth/status", async (route) => {
    await route.fulfill(jsonResponse({ root_initialized: true }));
  });

  await page.route("**/api/agent_nodes/bootstrap", async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(nodeJoinBootstrap));
      return;
    }
    await route.fallback();
  });

  await page.route("**/api/agent_nodes", async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(nodes));
      return;
    }
    await route.fallback();
  });

  await page.route("**/api/agents", async (route, request) => {
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        name: string;
        workdir: string;
        command: string;
        args?: string[];
        code_mode?: boolean;
      };
      const created: E2eAgentRecord = {
        id: `agent-forge-${agents.length + 1}`,
        name: payload.name,
        workdir: payload.workdir,
        command: payload.command,
        args: payload.args ?? [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: payload.code_mode ?? true,
        status: "idle",
        created_at: now,
        updated_at: now,
      };
      agents.push(created);
      await route.fulfill(jsonResponse(created));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(agents));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/agents\/[^/]+\/\.well-known\/agent-card$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const segments = url.pathname.split("/");
    const agentId = segments[segments.length - 3] ?? "";
    const agent = agents.find((item) => item.id === agentId);
    if (!agent) {
      await route.fulfill(jsonResponse({ error: "agent not found" }, 404));
      return;
    }
    const commandName = agent.command.split("/").pop() ?? agent.command;
    const acpProvider =
      commandName === "gemini"
        ? "gemini"
        : commandName === "kimi"
          ? "kimi"
          : "codex";
    const capabilityTags = ["team_mailbox_v1", "team_step_execution_v1"];
    if (agent.code_mode) capabilityTags.push("code_mode");
    if (
      agent.worktree_mode === "create_worktree" ||
      agent.worktree_mode === "reuse_worktree"
    ) {
      capabilityTags.push("git_worktree");
    }
    capabilityTags.push(`acp_${acpProvider}`);
    await route.fulfill(
      jsonResponse({
        card_id: `agenthub://agents/${agent.id}`,
        schema_version: "agenthub.a2a.discovery_card.v1",
        description: `AgentHub team member ${agent.name} (provider: ${acpProvider}) supports ${capabilityTags.join(", ")}`,
        identity: {
          agent_id: agent.id,
          name: agent.name,
          status: agent.status,
        },
        runtime: {
          acp_provider: acpProvider,
          code_mode: agent.code_mode,
          worktree_mode: agent.worktree_mode,
          worktree_repo: agent.worktree_repo ?? null,
          worktree_ref: agent.worktree_ref ?? null,
        },
        capability_tags: capabilityTags,
      })
    );
  });

  await page.route("**/api/teams", async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(teams));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as CreateTeamPayload;
      createTeamPayload = payload;
      const created: TeamDefinitionRecord = {
        id: `team-e2e-${teams.length + 1}`,
        name: payload.name,
        description: payload.description ?? null,
        spec: payload.spec,
        created_at: now,
        updated_at: now,
      };
      teams.push(created);
      teamRuntimeStateById.set(created.id, {
        status: payload.spec.members.length > 0 ? "running" : "stopped",
      });
      await route.fulfill(jsonResponse(created));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.split("/").pop() ?? "";
    if (request.method() === "GET") {
      const found = teams.find((team) => team.id === teamId);
      if (!found) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      await route.fulfill(jsonResponse(found));
      return;
    }
    if (request.method() === "DELETE") {
      const index = teams.findIndex((team) => team.id === teamId);
      if (index < 0) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      const [deleted] = teams.splice(index, 1);
      teamRuntimeStateById.delete(teamId);
      await route.fulfill(jsonResponse(deleted));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/spec$/, async (route, request) => {
    if (request.method() !== "PUT") {
      await route.fallback();
      return;
    }
    const teamId = request.url().match(/\/api\/teams\/([^/]+)\/spec$/)?.[1] ?? "";
    const index = teams.findIndex((team) => team.id === teamId);
    if (index < 0) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    const payload = request.postDataJSON() as UpdateTeamSpecPayload;
    if (payload.expected_updated_at !== teams[index].updated_at) {
      await route.fulfill(jsonResponse({ error: "team spec changed" }, 409));
      return;
    }
    updateSpecPayloads.push({ teamId, payload });
    const updated: TeamDefinitionRecord = {
      ...teams[index],
      spec: payload.spec,
      updated_at: now + updateSpecPayloads.length,
    };
    teams[index] = updated;
    teamRuntimeStateById.set(teamId, {
      status: payload.spec.members.length > 0 ? "running" : "stopped",
    });
    await route.fulfill(jsonResponse(updated));
  });

  await page.route(/\/api\/teams\/[^/]+\/runtime$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runtime$/)?.[1] ?? "";
    const team = teams.find((item) => item.id === teamId);
    if (!team) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(buildTeamRuntime(team)));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/(start|stop)$/, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const matched = url.pathname.match(/\/api\/teams\/([^/]+)\/(start|stop)$/);
    const teamId = matched?.[1] ?? "";
    const action = matched?.[2] ?? "";
    const team = teams.find((item) => item.id === teamId);
    if (!team) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    teamRuntimeStateById.set(team.id, {
      status: action === "start" ? "running" : "stopped",
    });
    await route.fulfill(
      jsonResponse({
        team_id: team.id,
        status: action === "start" ? "running" : "stopped",
        members: team.spec.members.map((member) => ({
          member_id: member.member_id,
          session_id: `session-${team.id}-${member.member_id}`,
          action: action === "start" ? "started" : "stopped",
        })),
      })
    );
  });

  await page.route(/\/api\/teams\/[^/]+\/tasks(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
    if (!teamId) {
      await route.fulfill(jsonResponse({ error: "team id missing" }, 400));
      return;
    }
    if (request.method() === "GET") {
      const tasks = ensureTasks(teamId);
      await route.fulfill(jsonResponse(tasks));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        title?: string;
        created_by_actor_id?: string;
        topic?: string;
        conversation_mode?: "to_coordinator" | "to_member" | "group_chat";
      };
      const current = ensureTasks(teamId);
      const nextIndex = (taskCounterByTeamId.get(teamId) ?? current.length) + 1;
      taskCounterByTeamId.set(teamId, nextIndex);
      const taskId = `task-${teamId}-${nextIndex}`;
      const createdAt = now + 100 + nextIndex;
      const createdTask: TeamTaskRecord = {
        id: taskId,
        team_id: teamId,
        title: payload.title?.trim() || `Conversation ${nextIndex}`,
        status: "open",
        created_by_actor_id: payload.created_by_actor_id ?? `user:${auth.userId}`,
        context: {
          topic: payload.topic ?? null,
          conversation_mode: payload.conversation_mode ?? "to_coordinator",
        },
        created_at: createdAt,
        updated_at: createdAt,
      };
      tasksByTeamId.set(teamId, [createdTask, ...current.filter((item) => item.id !== taskId)]);
      taskMessagesById.set(taskId, []);
      await route.fulfill(jsonResponse({ task: createdTask }));
      return;
    }
    await route.fallback();
  });

  await page.route(
    /\/api\/teams\/[^/]+\/tasks\/[^/]+\/messages(?:\?.*)?$/,
    async (route, request) => {
      const url = new URL(request.url());
      const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
      const taskId =
        url.pathname.match(/\/tasks\/([^/]+)\/messages(?:\?.*)?$/)?.[1] ?? "";
      if (!teamId || !taskId) {
        await route.fulfill(jsonResponse({ error: "path params missing" }, 400));
        return;
      }
      const tasks = ensureTasks(teamId);
      const task = tasks.find((item) => item.id === taskId);
      if (!task) {
        await route.fulfill(jsonResponse({ error: "task not found" }, 404));
        return;
      }
      const existingMessages = taskMessagesById.get(taskId) ?? [];
      if (request.method() === "GET") {
        await route.fulfill(jsonResponse(existingMessages));
        return;
      }
      if (request.method() === "POST") {
        const payload = request.postDataJSON() as {
          from_actor_id?: string;
          to_actor_id?: string | null;
          route?: "to_coordinator" | "to_member" | "group_chat";
          payload?: unknown;
        };
        const nextMessageId =
          (existingMessages.length > 0
            ? existingMessages[existingMessages.length - 1]?.message_id ?? 0
            : 0) + 1;
        const createdMessage: TeamConversationMessageRecord = {
          message_id: nextMessageId,
          conversation_id: `conversation-${taskId}`,
          task_id: taskId,
          from_actor_id: payload.from_actor_id ?? `user:${auth.userId}`,
          to_actor_id:
            payload.to_actor_id ??
            (payload.route === "to_member" ? "agent-worker-1" : "agent-coordinator-1"),
          route: payload.route ?? "to_coordinator",
          payload: payload.payload ?? { type: "chat_message", text: "" },
          created_at: now + 200 + nextMessageId,
        };
        const nextMessages = [...existingMessages, createdMessage];
        taskMessagesById.set(taskId, nextMessages);
        await route.fulfill(jsonResponse(createdMessage));
        return;
      }
      await route.fallback();
    }
  );

  await page.route(
    /\/api\/teams\/[^/]+\/tasks\/[^/]+\/compile_run_preview$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const url = new URL(request.url());
      const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
      const taskId =
        url.pathname.match(/\/tasks\/([^/]+)\/compile_run_preview$/)?.[1] ?? "";
      if (!teamId || !taskId) {
        await route.fulfill(jsonResponse({ error: "path params missing" }, 400));
        return;
      }
      const task = ensureTasks(teamId).find((item) => item.id === taskId);
      if (!task) {
        await route.fulfill(jsonResponse({ error: "task not found" }, 404));
        return;
      }
      const team = teams.find((item) => item.id === teamId);
      const coordinatorMemberId = team?.spec.coordinator_member_id ?? "agent-coordinator-1";
      const workerMemberId =
        team?.spec.members.find((member) => member.role === "worker")?.member_id ??
        coordinatorMemberId;
      const messageList = taskMessagesById.get(taskId) ?? [];
      const latestMessageId = messageList.length > 0 ? messageList[messageList.length - 1]?.message_id ?? 0 : 0;
      await route.fulfill(
        jsonResponse({
          task_id: taskId,
          conversation_id: `conversation-${taskId}`,
          run_payload: {
            context_id: `ctx-${taskId}`,
            input: {
              task_compile_version: 1,
              task_id: taskId,
              conversation_id: `conversation-${taskId}`,
              tool_name: "tiny-json-cli",
              objective: task.title,
              task_list: [
                "Define tiny CLI interface",
                "Implement parse/format commands",
                "Add tests and usage docs",
              ],
            },
          },
          plan: {
            task_list: [
              "Define tiny CLI interface",
              "Implement parse/format commands",
              "Add tests and usage docs",
            ],
            acceptance_criteria: [
              "CLI can parse and pretty-print JSON",
              "Unit tests pass for main happy paths",
            ],
            deadline: "2026-03-20",
            step_template: [
              {
                step_key: "coordinator_plan",
                member_id: coordinatorMemberId,
                role: "coordinator",
                depends_on: [],
              },
              {
                step_key: "worker_build_tool",
                member_id: workerMemberId,
                role: "worker",
                depends_on: ["coordinator_plan"],
              },
              {
                step_key: "coordinator_synthesize",
                member_id: coordinatorMemberId,
                role: "coordinator",
                depends_on: ["worker_build_tool"],
              },
            ],
            role_assignments: [
              {
                member_id: coordinatorMemberId,
                role: "coordinator",
                step_keys: ["coordinator_plan", "coordinator_synthesize"],
              },
              {
                member_id: workerMemberId,
                role: "worker",
                step_keys: ["worker_build_tool"],
              },
            ],
            source_message_id: latestMessageId,
          },
        })
      );
    }
  );

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().split("/").pop() ?? "";
    await route.fulfill(jsonResponse(buildSyntheticRun(runId)));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/steps$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/snapshot(?:\?.*)?$/,
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const runId =
        request.url().match(/\/api\/teams\/runs\/([^/]+)\/snapshot/)?.[1] ?? "";
      const run = buildSyntheticRun(runId);
      await route.fulfill(jsonResponse(buildSyntheticSnapshot(run)));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/inbox(?:\?.*)?$/,
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/inbox/)?.[1] ?? "";
      await route.fulfill(jsonResponse(ensureMailboxMessages(runId)));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/send$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/send/)?.[1] ?? "";
      const payload = request.postDataJSON() as {
        from_actor_id?: string;
        to_actor_id?: string;
        channel?: string;
        transport?: "local" | "remote";
        route?: Record<string, unknown> | null;
        payload?: unknown;
      };
      const messages = ensureMailboxMessages(runId);
      const nextMessageId =
        (messages[messages.length - 1]?.message_id ?? runEventCounterByRunId.get(runId) ?? 0) + 1;
      runEventCounterByRunId.set(runId, nextMessageId);
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id ?? `user:${auth.userId}`,
        from_actor_kind:
          (payload.from_actor_id ?? "").startsWith("user:") ? "human" : "agent",
        to_actor_id: payload.to_actor_id ?? "agent-coordinator-1",
        to_actor_kind:
          (payload.to_actor_id ?? "agent-coordinator-1").startsWith("user:")
            ? "human"
            : "agent",
        channel: payload.channel ?? "default",
        transport: payload.transport ?? "local",
        route: payload.route ?? null,
        payload: payload.payload ?? {},
        status: "pending",
        created_at: now + nextMessageId,
        delivered_at: null,
      };
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/\d+\/ack$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/\d+\/ack/)?.[1] ?? "";
      const messageId = Number(request.url().match(/\/messages\/(\d+)\/ack$/)?.[1] ?? 0);
      const messages = ensureMailboxMessages(runId);
      const found = messages.find((message) => message.message_id === messageId);
      if (!found) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      const delivered: TeamActorMessageRecord = {
        ...found,
        status: "delivered",
        delivered_at: now + messageId + 1,
      };
      const next = messages.map((message) =>
        message.message_id === messageId ? delivered : message
      );
      mailboxMessagesByRunId.set(runId, next);
      await route.fulfill(jsonResponse(delivered));
    }
  );

  // --- Channel routes ---
  const teamChannelsByTeamId = new Map<string, Array<{ channel_id: string; description?: string | null; task_id?: string | null; team_id: string; created_at: number; updated_at: number }>>();
  const ensureChannels = (teamId: string) => {
    if (!teamChannelsByTeamId.has(teamId)) {
      teamChannelsByTeamId.set(teamId, []);
    }
    return teamChannelsByTeamId.get(teamId)!;
  };

  await page.route(/\/api\/teams\/[^/]+\/channels(?:\?.*)?$/, async (route, request) => {
    const teamId = request.url().match(/\/api\/teams\/([^/]+)\/channels/)?.[1] ?? "";
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(ensureChannels(teamId)));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as { channel_id: string; description?: string | null };
      const channels = ensureChannels(teamId);
      if (channels.some((ch) => ch.channel_id === payload.channel_id)) {
        await route.fulfill(jsonResponse({ error: "channel already exists" }, 409));
        return;
      }
      const created = {
        channel_id: payload.channel_id,
        description: payload.description ?? null,
        task_id: null,
        team_id: teamId,
        created_at: now,
        updated_at: now,
      };
      channels.push(created);
      await route.fulfill(jsonResponse(created));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/channels\/[^/]+$/, async (route, request) => {
    const m = request.url().match(/\/api\/teams\/([^/]+)\/channels\/([^/]+)$/);
    if (!m || request.method() !== "DELETE") {
      await route.fallback();
      return;
    }
    const teamId = m[1];
    const channelId = m[2];
    const channels = ensureChannels(teamId);
    const idx = channels.findIndex((ch) => ch.channel_id === channelId);
    if (idx === -1) {
      await route.fulfill(jsonResponse({ error: "channel not found" }, 404));
      return;
    }
    const [deleted] = channels.splice(idx, 1);
    await route.fulfill(jsonResponse(deleted));
  });

  // Thread reply route
  await page.route(
    /\/api\/teams\/[^/]+\/channels\/[^/]+\/threads\/\d+\/replies$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const payload = request.postDataJSON() as { text: string; mention_actor_ids?: string[] };
      await route.fulfill(jsonResponse({
        message_id: 1000 + Math.floor(Math.random() * 9000),
        text: payload.text,
        created_at: now,
      }));
    }
  );

  return {
    now,
    auth,
    agents,
    nodes,
    nodeJoinBootstrap,
    teams,
    getCreatePayload: () => createTeamPayload,
    getUpdateSpecPayloads: () => updateSpecPayloads,
  };
}
