import { expect, test, testLocalLlm } from "./coverage";
import {
  type TeamActorMessageRecord,
  type TeamRunRecord,
  buildTeamRun,
  createTeamFromModal,
  enableDeveloperMode,
  gotoTeams,
  jsonResponse,
  mockTeamPageApis,
  openAdvancedView,
  openKanbanDeveloperTools,
  openMainTeamAction,
  openTeamFromSelector,
  selectAgentFromSidebar,
  selectPrimaryTeamEntryFromSidebar,
  teamSelectorPanel,
} from "./team_page_helpers";

test("team debug run ops compiles task preview and applies payload to create-run form", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const teamId = "team-compile";
  const teamCreatedAt = fixture.now + 120;
  const previewResponse = {
    task_id: "task-compile-1",
    conversation_id: "conversation-compile-1",
    run_payload: {
      context_id: "ctx-task-compile-1",
      input: {
        task_compile_version: 1,
        task_id: "task-compile-1",
        task_list: ["Implement compile preview", "Wire run ops"],
      },
    },
    plan: {
      task_list: ["Implement compile preview", "Wire run ops"],
      acceptance_criteria: ["Compile payload is deterministic"],
      deadline: "2026-03-08",
      step_template: [
        {
          step_key: "leader_plan",
          member_id: "planner",
          role: "leader",
          depends_on: [],
        },
      ],
      role_assignments: [
        {
          member_id: "planner",
          role: "leader",
          step_keys: ["leader_plan"],
        },
      ],
      source_message_id: 12,
    },
  };
  const compileRequests: Array<{ context_id?: string }> = [];

  fixture.teams.push({
    id: teamId,
    name: "Compile Team",
    description: "compile preview e2e",
    spec: {
      leader_member_id: "planner",
      members: [{ member_id: "planner", role: "leader", model: "codex" }],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await page.route(
    new RegExp(`/api/teams/${teamId}/tasks/[^/]+/compile_run_preview$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const payload = request.postDataJSON() as { context_id?: string };
      compileRequests.push(payload);
      await route.fulfill(jsonResponse(previewResponse));
    }
  );

  await gotoTeams(page);
  await openTeamFromSelector(page, "Compile Team");
  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);

  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect.poll(() => compileRequests.length).toBe(1);

  const compilePreview = page.locator('[data-team-compile-preview="true"]');
  await expect(compilePreview).toBeVisible();
  await expect(compilePreview).toContainText("conversation-compile-1");
  await expect(compilePreview).toContainText("ctx-task-compile-1");
  expect(compileRequests).toEqual([{}]);

  await page.getByRole("button", { name: "Use Payload in Create Run" }).click();
  await openAdvancedView(page, "Debug");
  await expect(
    page.getByPlaceholder("context_id (optional, auto-generated when empty)")
  ).toHaveValue("ctx-task-compile-1");
  await expect(page.getByLabel("Run input JSON")).toContainText(
    '"task_id": "task-compile-1"'
  );
});

test("team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const teamId = "team-chat-first";
  const runId = "run-chat-first-1";
  const teamCreatedAt = fixture.now + 180;
  const runCreatedAt = fixture.now + 260;
  const previewResponse = {
    task_id: "task-chat-1",
    conversation_id: "conversation-chat-1",
    run_payload: {
      context_id: "ctx-chat-first-1",
      input: {
        task_compile_version: 1,
        task_id: "task-chat-1",
        conversation_id: "conversation-chat-1",
        task_list: [
          "Negotiate scope with leader",
          "Worker implements endpoint",
          "Leader synthesizes final deliverable",
        ],
      },
    },
    plan: {
      task_list: [
        "Negotiate scope with leader",
        "Worker implements endpoint",
        "Leader synthesizes final deliverable",
      ],
      acceptance_criteria: ["Endpoint implemented", "Final summary delivered"],
      deadline: "2026-03-12",
      step_template: [
        {
          step_key: "leader_plan",
          member_id: "agent-leader-1",
          role: "leader",
          depends_on: [],
        },
        {
          step_key: "worker_execute",
          member_id: "agent-worker-1",
          role: "worker",
          depends_on: ["leader_plan"],
        },
        {
          step_key: "leader_synthesize",
          member_id: "agent-leader-1",
          role: "leader",
          depends_on: ["worker_execute"],
        },
      ],
      role_assignments: [
        {
          member_id: "agent-leader-1",
          role: "leader",
          step_keys: ["leader_plan", "leader_synthesize"],
        },
        {
          member_id: "agent-worker-1",
          role: "worker",
          step_keys: ["worker_execute"],
        },
      ],
      source_message_id: 18,
    },
  };

  fixture.teams.push({
    id: teamId,
    name: "Chat First Team",
    description: "chat-first e2e flow",
    spec: {
      leader_member_id: "agent-leader-1",
      members: [
        { member_id: "agent-leader-1", role: "leader", model: "codex" },
        { member_id: "agent-worker-1", role: "worker", model: "gemini" },
      ],
      steps: [
        { step_key: "leader_plan" },
        { step_key: "worker_execute" },
        { step_key: "leader_synthesize" },
      ],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  let activeRun: TeamRunRecord | null = null;
  let nextMessageId = 50;
  const createRunRequests: Array<{ context_id?: string; input?: unknown }> = [];
  const sentAcpInputs: Array<{
    agent_id: string;
    input: string;
    session_id?: string;
  }> = [];
  const messages: TeamActorMessageRecord[] = [
    {
      message_id: 1,
      run_id: runId,
      from_actor_id: "agent-leader-1",
      to_actor_id: "agent-worker-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "chat_message",
        text: "Please implement endpoint scaffolding and tests.",
      },
      status: "pending",
      created_at: runCreatedAt + 1,
      delivered_at: null,
    },
    {
      message_id: 2,
      run_id: runId,
      from_actor_id: "agent-worker-1",
      to_actor_id: "agent-leader-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "worker_status",
        status: "done",
        result: "Endpoint and tests are complete.",
        evidence: ["go test ./..."],
      },
      status: "pending",
      created_at: runCreatedAt + 2,
      delivered_at: null,
    },
  ];

  const runEvents = [
    {
      event_id: 201,
      run_id: runId,
      step_id: "step-worker-execute",
      event_type: "step_completed",
      ts: runCreatedAt + 10,
      payload: {
        step_key: "worker_execute",
        summary: "Worker implementation completed with tests.",
      },
    },
    {
      event_id: 202,
      run_id: runId,
      step_id: "step-leader-synthesize",
      event_type: "leader_synthesized",
      ts: runCreatedAt + 20,
      payload: {
        final_deliverable: "Final deliverable prepared and returned to user.",
      },
    },
  ];

  const buildSnapshot = () => {
    if (!activeRun) {
      return null;
    }
    return {
      run: activeRun,
      team: fixture.teams.find((team) => team.id === teamId),
      leader_member_id: "agent-leader-1",
      members: [
        {
          member_id: "agent-leader-1",
          role: "leader",
          model: "codex",
          prompt: "leader prompt",
          skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
          pending_inbox_count: messages.filter(
            (message) =>
              message.to_actor_id === "agent-leader-1" &&
              message.status === "pending"
          ).length,
          status: "working",
          latest_step: {
            id: "step-leader-synthesize",
            run_id: runId,
            step_key: "leader_synthesize",
            member_id: "agent-leader-1",
            remote_task_id: "task-leader-1",
            status: "working",
            attempt: 1,
            depends_on: ["worker_execute"],
            input: {},
            output: null,
            error_text: null,
            started_at: runCreatedAt + 8,
            ended_at: null,
          },
          session_status: "working",
        },
        {
          member_id: "agent-worker-1",
          role: "worker",
          model: "gemini",
          prompt: "worker prompt",
          skills: ["agenthub-actor-runtime", "team-worker-executor"],
          pending_inbox_count: messages.filter(
            (message) =>
              message.to_actor_id === "agent-worker-1" &&
              message.status === "pending"
          ).length,
          status: "working",
          latest_step: {
            id: "step-worker-execute",
            run_id: runId,
            step_key: "worker_execute",
            member_id: "agent-worker-1",
            remote_task_id: "task-worker-1",
            status: "completed",
            attempt: 1,
            depends_on: ["leader_plan"],
            input: {},
            output: { summary: "done" },
            error_text: null,
            started_at: runCreatedAt + 4,
            ended_at: runCreatedAt + 9,
          },
          session_status: "idle",
        },
      ],
      steps: [],
      latest_events: runEvents,
      mailbox: {
        pending: messages.filter((message) => message.status === "pending").length,
        delivered: messages.filter((message) => message.status === "delivered").length,
        dead_letter: 0,
        recent_messages: [...messages].sort((left, right) => left.message_id - right.message_id),
      },
    };
  };

  await page.route(
    new RegExp(`/api/teams/${teamId}/tasks/[^/]+/compile_run_preview$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      await route.fulfill(jsonResponse(previewResponse));
    }
  );

  await page.route(new RegExp(`/api/teams/${teamId}/runs(?:\\?.*)?$`), async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(activeRun ? [activeRun] : []));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as { context_id?: string; input?: unknown };
      createRunRequests.push(payload);
      activeRun = {
        id: runId,
        team_id: teamId,
        context_id: payload.context_id ?? "ctx-chat-first-fallback",
        status: "working",
        input: (payload.input as Record<string, unknown>) ?? {},
        created_at: runCreatedAt,
        started_at: runCreatedAt + 1,
        ended_at: null,
      };
      await route.fulfill(jsonResponse(activeRun));
      return;
    }
    await route.fallback();
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(activeRun));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/steps$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/events(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(activeRun ? runEvents : []));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const snapshot = buildSnapshot();
      if (!snapshot) {
        await route.fulfill(jsonResponse({ error: "snapshot unavailable" }, 404));
        return;
      }
      await route.fulfill(jsonResponse(snapshot));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const url = new URL(request.url());
      const actorId = url.searchParams.get("actor_id") ?? "";
      const includeDelivered = url.searchParams.get("include_delivered") === "true";
      const inboxMessages = messages
        .filter((message) => message.to_actor_id === actorId)
        .filter((message) => includeDelivered || message.status !== "delivered")
        .sort((left, right) => left.message_id - right.message_id);
      await route.fulfill(jsonResponse(inboxMessages));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/send$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const payload = request.postDataJSON() as {
        from_actor_id: string;
        to_actor_id: string;
        payload: unknown;
      };
      sentMessagePayloads.push({
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        payload: payload.payload,
      });
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        channel: "default",
        transport: "local",
        route: null,
        payload: payload.payload,
        status: "pending",
        created_at: runCreatedAt + nextMessageId,
        delivered_at: null,
      };
      nextMessageId += 1;
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/\\d+/ack$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const messageIdMatch = request.url().match(/messages\/(\d+)\/ack$/);
      const messageId = Number(messageIdMatch?.[1] ?? "0");
      const message = messages.find((item) => item.message_id === messageId);
      if (!message) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      message.status = "delivered";
      message.delivered_at = runCreatedAt + messageId;
      await route.fulfill(jsonResponse(message));
    }
  );

  await page.route(/\/api\/agents\/[^/]+\/input$/, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fallback();
      return;
    }
    const match = request.url().match(/\/api\/agents\/([^/]+)\/input$/);
    const agentId = decodeURIComponent(match?.[1] ?? "");
    const payload = request.postDataJSON() as {
      input: string;
      session_id?: string;
    };
    sentAcpInputs.push({
      agent_id: agentId,
      input: payload.input,
      session_id: payload.session_id,
    });
    await route.fulfill(jsonResponse({ status: "ok" }));
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Chat First Team");
  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);

  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect(page.getByText("conversation-chat-1", { exact: true })).toBeVisible();
  await expect(page.getByText("Negotiate scope with leader")).toBeVisible();

  await page.getByRole("button", { name: "Create Run from Preview" }).click();
  await openMainTeamAction(page, "Execution Runs");
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(runId);
  expect(createRunRequests).toHaveLength(1);
  expect(createRunRequests[0]).toMatchObject({
    context_id: "ctx-chat-first-1",
  });
  expect(
    (createRunRequests[0]?.input as { task_id?: string } | undefined)
      ?.task_id
  ).toBe("task-chat-1");

  await selectAgentFromSidebar(page, "Worker Agent");
  const agentInput = page.getByPlaceholder(/Send input|Type a message \(tap Send/);
  await expect(agentInput).toBeVisible();
  await agentInput.fill("Please include migration notes in the final report.");
  await page.getByRole("button", { name: "Send input", exact: true }).click();
  await expect
    .poll(() => sentAcpInputs.length, { timeout: 15_000 })
    .toBe(1);
  expect(sentAcpInputs).toHaveLength(1);
  expect(sentAcpInputs[0]).toMatchObject({
    agent_id: "agent-worker-1",
    input: "Please include migration notes in the final report.",
  });

  await openAdvancedView(page, "Member Console");
  await expect(page.locator(".teams-step-body")).toContainText("a2a_discovery_card");
  await expect(page.locator(".teams-step-body")).not.toContainText("Loading discovery card...");
  await expect(page.locator(".teams-step-body")).toContainText("acp_gemini");

  await openMainTeamAction(page, "Execution Runs");
  await openAdvancedView(page, "Events");
  await expect(page.locator(".teams-event-list")).toContainText(
    "Final deliverable prepared and returned to user."
  );
});

testLocalLlm("team conversation-first integration supports virtual team tiny-tool delivery flow [local-llm]", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const runId = "run-virtual-tool-1";
  let activeRun: TeamRunRecord | null = null;
  const createRunRequests: Array<{ context_id?: string; input?: unknown }> = [];

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/)?.[1] ?? "";
    if (!teamId) {
      await route.fulfill(jsonResponse({ error: "team id missing" }, 400));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(
        jsonResponse(activeRun && activeRun.team_id === teamId ? [activeRun] : [])
      );
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as { context_id?: string; input?: unknown };
      createRunRequests.push(payload);
      activeRun = {
        id: runId,
        team_id: teamId,
        context_id: payload.context_id ?? "ctx-virtual-tool-fallback",
        status: "working",
        input: (payload.input as Record<string, unknown>) ?? {},
        created_at: fixture.now + 900,
        started_at: fixture.now + 901,
        ended_at: null,
      };
      await route.fulfill(jsonResponse(activeRun));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(activeRun));
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

  await page.route(/\/api\/teams\/runs\/[^/]+\/snapshot(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "snapshot unavailable" }, 404));
      return;
    }
    const team = fixture.teams.find((item) => item.id === activeRun?.team_id) ?? null;
    await route.fulfill(
      jsonResponse({
        run: activeRun,
        team,
        leader_member_id: team?.spec.leader_member_id ?? "tool-leader",
        members: [],
        steps: [],
        latest_events: [],
        mailbox: {
          pending: 0,
          delivered: 0,
          dead_letter: 0,
          recent_messages: [],
        },
      })
    );
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: "virtual-tool-team",
    goal: "Conversation-first tiny tool delivery flow.",
  });

  const virtualToolSpec = {
    spec_version: 1,
    entrypoint: "leader_plan",
    leader_member_id: "tool-leader",
    members: [
      { member_id: "tool-leader", role: "leader", model: "codex" },
      { member_id: "tool-worker", role: "worker", model: "gemini" },
    ],
    steps: [
      { step_key: "leader_plan", member_id: "tool-leader", depends_on: [] },
      { step_key: "worker_build_tool", member_id: "tool-worker", depends_on: ["leader_plan"] },
      {
        step_key: "leader_synthesize",
        member_id: "tool-leader",
        depends_on: ["worker_build_tool"],
      },
    ],
  };
  fixture.agents.push(
    {
      id: "tool-leader",
      name: "tool-leader",
      workdir: "/workspace/tool-leader",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 210,
      updated_at: fixture.now + 210,
    },
    {
      id: "tool-worker",
      name: "tool-worker",
      workdir: "/workspace/tool-worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 211,
      updated_at: fixture.now + 211,
    }
  );
  const virtualToolTeam = fixture.teams.find((team) => team.name === "virtual-tool-team");
  if (!virtualToolTeam) {
    throw new Error("virtual-tool-team was not created");
  }
  virtualToolTeam.spec = virtualToolSpec;
  virtualToolTeam.updated_at += 1;
  await gotoTeams(page);
  await openTeamFromSelector(page, "virtual-tool-team");

  await expect(page.getByRole("heading", { name: "# all", exact: true })).toBeVisible();
  await page
    .getByPlaceholder("Message #all")
    .fill("Please build a tiny JSON CLI with parse and pretty-print commands.");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.locator(".teams-main")).toContainText(
    "Please build a tiny JSON CLI with parse and pretty-print commands."
  );

  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);
  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect(page.locator(".teams-step-body")).toContainText("tiny-json-cli");
  await expect(page.locator(".teams-step-body")).toContainText(
    "Define tiny CLI interface"
  );

  await page.getByRole("button", { name: "Create Run from Preview" }).click();
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(runId);
  expect(createRunRequests).toHaveLength(1);
  expect(
    (createRunRequests[0]?.input as { tool_name?: string } | undefined)?.tool_name
  ).toBe("tiny-json-cli");
  expect(
    (createRunRequests[0]?.input as { objective?: string } | undefined)?.objective
  ).toBe("Build tiny JSON CLI");
});

test("team mailbox IM mode supports conversation focus, unread, auto-follow and advanced controls", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-mailbox";
  const runId = "run-mailbox-1";
  const teamCreatedAt = fixture.now + 100;
  const runCreatedAt = fixture.now + 200;

  fixture.teams.push({
    id: teamId,
    name: "Team Mailbox",
    description: "mailbox im test",
    spec: {
      leader_member_id: "agent-leader-1",
      members: [
        { member_id: "agent-leader-1", role: "leader", model: "codex" },
        { member_id: "agent-worker-1", role: "worker", model: "gemini" },
        { member_id: "agent-worker-2", role: "worker", model: "kimi" },
      ],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  const runRecord: TeamRunRecord = {
    id: runId,
    team_id: teamId,
    context_id: "ctx-mailbox",
    status: "working",
    input: { prompt: "mailbox test" },
    created_at: runCreatedAt,
    started_at: runCreatedAt + 1,
    ended_at: null,
  };

  const now = fixture.now + 1_000;
  const messages: TeamActorMessageRecord[] = [];
  for (let index = 1; index <= 36; index += 1) {
    const fromWorker = index % 2 === 0;
    messages.push({
      message_id: index,
      run_id: runId,
      from_actor_id: fromWorker ? "agent-worker-1" : "agent-leader-1",
      to_actor_id: fromWorker ? "agent-leader-1" : "agent-worker-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "chat_message",
        text: `worker1-${index}`,
      },
      status: "pending",
      created_at: now + index,
      delivered_at: null,
    });
  }
  messages.push({
    message_id: 80,
    run_id: runId,
    from_actor_id: "agent-worker-2",
    to_actor_id: "agent-leader-1",
    channel: "default",
    transport: "local",
    route: null,
    payload: { type: "chat_message", text: "worker2-unread" },
    status: "pending",
    created_at: now + 80,
    delivered_at: null,
  });
  let nextMessageId = 120;

  const counters = {
    events: 0,
    snapshot: 0,
    inbox: 0,
    send: 0,
  };

  const computePendingInboxCount = (actorId: string): number =>
    messages.filter(
      (message) => message.to_actor_id === actorId && message.status === "pending"
    ).length;

  const buildSnapshot = () => ({
    run: runRecord,
    team: fixture.teams.find((team) => team.id === teamId),
    leader_member_id: "agent-leader-1",
    members: [
      {
        member_id: "agent-leader-1",
        role: "leader",
        model: "codex",
        prompt: "leader",
        skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
        pending_inbox_count: computePendingInboxCount("agent-leader-1"),
        status: "working",
        latest_step: null,
        session_status: "working",
      },
      {
        member_id: "agent-worker-1",
        role: "worker",
        model: "gemini",
        prompt: "worker-1",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
        pending_inbox_count: computePendingInboxCount("agent-worker-1"),
        status: "working",
        latest_step: null,
        session_status: "idle",
      },
      {
        member_id: "agent-worker-2",
        role: "worker",
        model: "kimi",
        prompt: "worker-2",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
        pending_inbox_count: computePendingInboxCount("agent-worker-2"),
        status: "working",
        latest_step: null,
        session_status: "idle",
      },
    ],
    steps: [],
    latest_events: [
      {
        event_id: 1,
        run_id: runId,
        step_id: null,
        event_type: "run_working",
        ts: now,
        payload: { status: "working" },
      },
    ],
    mailbox: {
      pending: messages.filter((message) => message.status === "pending").length,
      delivered: messages.filter((message) => message.status === "delivered").length,
      dead_letter: 0,
      recent_messages: [...messages].sort((a, b) => a.message_id - b.message_id),
    },
  });

  await page.route(new RegExp(`/api/teams/${teamId}/runs(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([runRecord]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(runRecord));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/events(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    counters.events += 1;
    await route.fulfill(jsonResponse([]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/steps$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      counters.snapshot += 1;
      await route.fulfill(jsonResponse(buildSnapshot()));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      counters.inbox += 1;
      const url = new URL(request.url());
      const actorId = url.searchParams.get("actor_id") ?? "";
      const includeDelivered = url.searchParams.get("include_delivered") === "true";
      const afterRaw = url.searchParams.get("after_id");
      const afterId = afterRaw ? Number(afterRaw) : null;
      const list = messages
        .filter((message) => message.to_actor_id === actorId)
        .filter((message) => includeDelivered || message.status !== "delivered")
        .filter((message) => (afterId == null ? true : message.message_id > afterId))
        .sort((left, right) => left.message_id - right.message_id);
      await route.fulfill(jsonResponse(list));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/send$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      counters.send += 1;
      const payload = request.postDataJSON() as {
        from_actor_id: string;
        to_actor_id: string;
        channel?: string;
        transport?: "local" | "remote";
        route?: Record<string, unknown> | null;
        payload: unknown;
      };
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        channel: payload.channel ?? "default",
        transport: payload.transport ?? "local",
        route: payload.route ?? null,
        payload: payload.payload,
        status: "pending",
        created_at: now + nextMessageId,
        delivered_at: null,
      };
      nextMessageId += 1;
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/\\d+/ack$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const messageIdMatch = request.url().match(/messages\/(\d+)\/ack$/);
      const messageId = Number(messageIdMatch?.[1] ?? "0");
      const message = messages.find((item) => item.message_id === messageId);
      if (!message) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      message.status = "delivered";
      message.delivered_at = now + messageId;
      await route.fulfill(jsonResponse(message));
    }
  );

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  const unreadFor = async (memberId: string): Promise<number> => {
    const badge = page
      .locator(".teams-chat-members .team-item", { hasText: `${memberId} (` })
      .locator(".teams-member-unread");
    if ((await badge.count()) === 0) {
      return 0;
    }
    const label = await badge.innerText();
    const match = label.match(/unread=(\d+)/);
    return match ? Number(match[1]) : 0;
  };

  await enableDeveloperMode(page);
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Mailbox");
  await openAdvancedView(page, "Execution Mailbox");
  await expect(page.locator(".teams-chat-shell")).toBeVisible();

  const unreadWorker2Before = await unreadFor("Worker Agent Two");
  expect(unreadWorker2Before).toBeGreaterThan(0);

  await page
    .locator(".teams-chat-members .team-item", { hasText: "Worker Agent (worker)" })
    .click();
  await expect(page.locator(".teams-chat-head")).toContainText(
    "Leader Agent → Worker Agent"
  );
  await page.getByRole("button", { name: "Jump to bottom" }).click();
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=on");
  await expect.poll(async () => unreadFor("Worker Agent")).toBe(0);
  expect(await unreadFor("Worker Agent Two")).toBeGreaterThan(0);

  await page.locator(".teams-chat-messages").evaluate((element) => {
    const target = element as HTMLElement;
    target.scrollTop = 0;
    target.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=off");

  await page
    .locator(".teams-chat-members .team-item", { hasText: "Worker Agent Two (worker)" })
    .click();
  await expect(page.locator(".teams-chat-head")).toContainText(
    "Leader Agent → Worker Agent Two"
  );

  const eventsBeforePolling = counters.events;
  const snapshotBeforePolling = counters.snapshot;
  const inboxBeforePolling = counters.inbox;
  await page.waitForTimeout(4500);
  expect(counters.events).toBe(eventsBeforePolling);
  expect(counters.snapshot).toBeGreaterThan(snapshotBeforePolling);
  expect(counters.inbox).toBeGreaterThan(inboxBeforePolling);

  await openAdvancedView(page, "Debug");
  await page.getByRole("button", { name: "Mailbox Raw" }).click();
  await expect(page.getByRole("heading", { name: "Send Message (JSON)" })).toBeVisible();

  const advancedPanel = page.locator(".teams-message-advanced .teams-message-panel").first();
  await advancedPanel.getByPlaceholder("from_actor_id").fill("agent-leader-1");
  await advancedPanel.getByPlaceholder("to_actor_id").fill("agent-worker-2");
  await advancedPanel
    .getByPlaceholder("payload JSON")
    .fill('{"type":"chat_message","text":"advanced-mailbox-ping"}');
  await advancedPanel.getByRole("button", { name: "Send Message" }).click();

  await openMainTeamAction(page, "Execution Runs");
  await openAdvancedView(page, "Overview");
  await page
    .locator(".teams-member-list .team-member-row", { hasText: "Worker Agent Two (worker)" })
    .click();
  await expect(page.locator(".teams-chat-messages")).toContainText("advanced-mailbox-ping");
  expect(counters.send).toBeGreaterThan(0);
});

test("team list supports deleting selected team", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push(
    {
      id: "team-delete-a",
      name: "Team Delete A",
      description: "first team",
      spec: {
        leader_member_id: "agent-leader-1",
        members: [{ member_id: "agent-leader-1", role: "leader", model: "codex" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now,
      updated_at: fixture.now,
    },
    {
      id: "team-delete-b",
      name: "Team Delete B",
      description: "second team",
      spec: {
        leader_member_id: "agent-worker-1",
        members: [{ member_id: "agent-worker-1", role: "leader", model: "gemini" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now + 1,
      updated_at: fixture.now + 1,
    }
  );

  await gotoTeams(page);
  await expect(
    teamSelectorPanel(page)
      .locator('[data-team-selector-entry="true"]', { hasText: "Team Delete A" })
      .first()
  ).toBeVisible();
  await expect(
    teamSelectorPanel(page)
      .locator('[data-team-selector-entry="true"]', { hasText: "Team Delete B" })
      .first()
  ).toBeVisible();
  await openTeamFromSelector(page, "Team Delete A");
  await openMainTeamAction(page, "Execution Runs");
  await expect(page.locator(".teams-main").getByText("Team Delete A", { exact: true })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete Team" }).click();

  await expect(teamSelectorPanel(page)).toBeVisible();
  await expect(
    teamSelectorPanel(page).locator('[data-team-selector-entry="true"]', { hasText: "Team Delete A" })
  ).toHaveCount(0);
  await expect(
    teamSelectorPanel(page)
      .locator('[data-team-selector-entry="true"]', { hasText: "Team Delete B" })
      .first()
  ).toBeVisible();
});

test("team run list resets filters on team switch and uses before_created_at cursor paging", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push(
    {
      id: "team-a",
      name: "Team A",
      description: "first team",
      spec: {
        leader_member_id: "agent-leader-1",
        members: [{ member_id: "agent-leader-1", role: "leader", model: "codex" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now,
      updated_at: fixture.now,
    },
    {
      id: "team-b",
      name: "Team B",
      description: "second team",
      spec: {
        leader_member_id: "agent-worker-1",
        members: [{ member_id: "agent-worker-1", role: "leader", model: "gemini" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now + 1,
      updated_at: fixture.now + 1,
    }
  );

  const runQueries: Array<{ teamId: string; status: string; before: number | null }> = [];
  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const teamIdMatch = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/);
    const teamId = teamIdMatch?.[1] ?? "";
    const status = url.searchParams.get("status") ?? "all";
    const beforeRaw = url.searchParams.get("before_created_at");
    const before = beforeRaw == null ? null : Number(beforeRaw);
    runQueries.push({ teamId, status, before });

    let payload: TeamRunRecord[] = [];
    if (teamId === "team-a" && status === "all") {
      payload = [buildTeamRun("team-a", "submitted", 500, 1)];
    } else if (teamId === "team-a" && status === "working" && before == null) {
      payload = Array.from({ length: 50 }, (_, index) =>
        buildTeamRun("team-a", "working", 300 - index, index)
      );
    } else if (teamId === "team-a" && status === "working" && before === 251) {
      payload = [buildTeamRun("team-a", "working", 250, 999)];
    } else if (teamId === "team-b" && status === "all") {
      payload = [buildTeamRun("team-b", "submitted", 450, 1)];
    } else if (teamId === "team-b" && status === "failed") {
      payload = [buildTeamRun("team-b", "failed", 400, 2)];
    }

    await route.fulfill(jsonResponse(payload));
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Team A");
  await openMainTeamAction(page, "Execution Runs");

  const runFilter = page.getByLabel("Run status filter");
  await expect(runFilter).toHaveValue("all");

  await runFilter.selectOption("working");
  await expect(runFilter).toHaveValue("working");
  const loadMoreRunsButton = page.getByRole("button", { name: "Load More" });
  await expect(loadMoreRunsButton).toBeEnabled();
  await loadMoreRunsButton.click();

  await expect
    .poll(() =>
      runQueries.some(
        (query) =>
          query.teamId === "team-a" &&
          query.status === "working" &&
          query.before === 251
      )
    )
    .toBe(true);

  await openTeamFromSelector(page, "Team B");
  if ((await runFilter.count()) === 0) {
    await openMainTeamAction(page, "Execution Runs");
  }
  await expect(runFilter).toHaveValue("all");
  await runFilter.selectOption("failed");
  await expect(runFilter).toHaveValue("failed");

  await openTeamFromSelector(page, "Team A");
  if ((await runFilter.count()) === 0) {
    await openMainTeamAction(page, "Execution Runs");
  }
  await expect(runFilter).toHaveValue("all");

  await openTeamFromSelector(page, "Team B");
  if ((await runFilter.count()) === 0) {
    await openMainTeamAction(page, "Execution Runs");
  }
  await expect(runFilter).toHaveValue("all");
  await expect(page.getByRole("alert")).toHaveCount(0);
});
