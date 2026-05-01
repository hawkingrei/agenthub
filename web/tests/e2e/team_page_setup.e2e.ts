import { expect, test } from "./coverage";
import {
  type CreateTeamPayload,
  type TeamRunRecord,
  buildTeamRun,
  clickSelectedTeamMenuItem,
  createTeamFromModal,
  createTeamMemberFromModal,
  enableDeveloperMode,
  expectAddAgentEntryVisible,
  expectTeamRuntimeBadge,
  gotoTeams,
  isTeamDetailReady,
  jsonResponse,
  mockTeamPageApis,
  openAdvancedView,
  openMainTeamAction,
  openSelectedTeamMenu,
  openTeamFromSelector,
  selectAgentFromSidebar,
  selectPrimaryTeamEntryFromSidebar,
  teamSelectorPanel,
} from "./team_page_helpers";

test("team runtime controls update shared runtime badge", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push({
    id: "team-runtime-controls",
    name: "runtime controls team",
    description: "runtime badge coverage",
    spec: {
      coordinator_member_id: "agent-coordinator-1",
      members: [
        { member_id: "agent-coordinator-1", role: "coordinator", description: "lead" },
        { member_id: "agent-worker-1", role: "worker", description: "worker" },
      ],
      steps: [{ step_key: "coordinator_plan" }, { step_key: "worker_exec" }],
    },
    created_at: fixture.now + 20,
    updated_at: fixture.now + 20,
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "runtime controls team");
  await expectTeamRuntimeBadge(page, "team running");

  await clickSelectedTeamMenuItem(page, "Stop Team");
  await expectTeamRuntimeBadge(page, "team stopped");

  await clickSelectedTeamMenuItem(page, "Start Team");
  await expectTeamRuntimeBadge(page, "team running");
});

test("team create flow stores mission metadata before member setup", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);

  await gotoTeams(page);

  await expect(teamSelectorPanel(page)).toBeVisible();
  await createTeamFromModal(page, {
    name: "quest-team",
    goal: "Build a goal-first team and add members afterward.",
  });
  await expect(page).toHaveURL(/\/teams\/.+/);
  await expect.poll(() => isTeamDetailReady(page)).toBe(true);
  await openSelectedTeamMenu(page);
  await expect(page.getByRole("menuitem", { name: "Add Agent", exact: true })).toBeVisible();
  await expect(page.getByText("No agents have joined this team yet.")).toBeVisible();

  const payload = fixture.getCreatePayload();
  expect(payload).not.toBeNull();
  const createdPayload = payload as CreateTeamPayload;
  expect(createdPayload.name).toBe("quest-team");
  expect(createdPayload.description).toBe("Build a goal-first team and add members afterward.");
  expect(createdPayload.spec).toEqual({
    spec_version: 1,
    members: [],
  });
});

test("team member setup adds the first agent and appends more agents through spec updates", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamName = "member-setup-team";

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: teamName,
    goal: "Create team first, then configure coordinator and worker profiles.",
  });

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/member-setup-coordinator",
    model: "codex",
    identity: "Principal planner and reviewer",
  });
  await openTeamFromSelector(page, teamName);
  await expectAddAgentEntryVisible(page, teamName);

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/member-setup-worker",
    model: "gemini",
    identity: "Implementation specialist",
  });

  const updates = fixture.getUpdateSpecPayloads();
  expect(updates).toHaveLength(2);
  expect(updates[0]?.payload.spec.coordinator_member_id).toBe("agent-forge-4");
  expect(updates[0]?.payload.spec.members.map((member) => member.role)).toEqual(["coordinator"]);
  expect(updates[1]?.payload.spec.members.map((member) => member.role)).toEqual([
    "coordinator",
    "worker",
  ]);
  const [coordinatorMember, workerMember] = updates[1]?.payload.spec.members ?? [];
  expect(coordinatorMember?.model).toBe("codex");
  expect(coordinatorMember?.skills).toBeUndefined();
  expect(workerMember?.model).toBe("gemini");
  expect(workerMember?.skills).toBeUndefined();
  expect(updates[1]?.payload.spec.steps?.map((step) => step.step_key)).toEqual([
    "coordinator_plan",
    "worker_1_agent_forge_5",
    "coordinator_synthesize",
  ]);
});

test("team create modal only captures mission metadata and points member setup to the next step", async ({
  page,
}) => {
  await mockTeamPageApis(page);
  await gotoTeams(page);

  await openMainTeamAction(page, "New Team");
  const dialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Start with the mission" })
    .last();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Team name")).toBeVisible();
  await expect(dialog.getByLabel("Team goal")).toBeVisible();
  await expect(dialog.getByRole("button", { name: /Add Agent|New Agent/ })).toHaveCount(0);
  await expect(dialog.getByText("Add agents afterward")).toBeVisible();
});

test("team page keeps single-column proportions on mobile viewport", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longCoordinatorId = `agent-coordinator-${"x".repeat(72)}`;
  const longWorkerId = `agent-worker-${"y".repeat(72)}`;
  fixture.teams.push({
    id: "team-mobile",
    name: "Team Mobile",
    description: "mobile layout regression guard",
    spec: {
      coordinator_member_id: longCoordinatorId,
      members: [
        { member_id: longCoordinatorId, role: "coordinator", model: "codex" },
        { member_id: longWorkerId, role: "worker", model: "gemini" },
      ],
      steps: [{ step_key: "coordinator_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Mobile");
  await openMainTeamAction(page, "Execution Runs");

  await expect(page.locator(".teams-main").getByText("Team Mobile", { exact: true })).toBeVisible();

  const layoutColumns = await page.locator(".teams-layout").evaluate((element) => {
    return window.getComputedStyle(element).gridTemplateColumns;
  });
  expect(layoutColumns.trim().split(/\s+/).length).toBe(1);

  const { runFilterWidth, runFilterParentWidth } = await page
    .getByLabel("Run status filter")
    .first()
    .evaluate((select) => {
      const parent = select.parentElement;
      return {
        runFilterWidth: select.getBoundingClientRect().width,
        runFilterParentWidth: parent?.getBoundingClientRect().width ?? 0,
      };
    });
  expect(runFilterWidth).toBeGreaterThan(runFilterParentWidth * 0.7);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("team page desktop keeps long metadata blocks non-overlapping", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longCoordinatorId = `coordinator-${"l".repeat(96)}`;
  const longWorkerId1 = `worker-a-${"w".repeat(88)}`;
  const longWorkerId2 = `worker-b-${"z".repeat(88)}`;
  const longPrompt = `prompt-${"p".repeat(420)}`;
  const teamId = "team-desktop";
  fixture.teams.push({
    id: teamId,
    name: "Team Desktop",
    description: "desktop overlap regression guard",
    spec: {
      coordinator_member_id: longCoordinatorId,
      members: [
        {
          member_id: longCoordinatorId,
          role: "coordinator",
          model: "codex",
          skills: ["agenthub-actor-runtime", "team-coordinator-orchestrator", `mcp-${"m".repeat(52)}`],
        },
        {
          member_id: longWorkerId1,
          role: "worker",
          model: "gemini",
          skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"a".repeat(52)}`],
        },
        {
          member_id: longWorkerId2,
          role: "worker",
          model: "kimi",
          skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"b".repeat(52)}`],
        },
      ],
      steps: [{ step_key: "coordinator_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });
  const runRecord = buildTeamRun(teamId, "working", fixture.now + 10, 1);
  const runId = runRecord.id;
  const runEvents: Array<Record<string, unknown>> = [];

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
    await route.fulfill(jsonResponse(runEvents));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      await route.fulfill(
        jsonResponse({
          run: runRecord,
          team: fixture.teams.find((team) => team.id === teamId),
          coordinator_member_id: longCoordinatorId,
          members: [
            {
              member_id: longCoordinatorId,
              role: "coordinator",
              model: "codex",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-coordinator-orchestrator", `mcp-${"m".repeat(52)}`],
              pending_inbox_count: 0,
              status: "working",
              latest_step: {
                id: "step-coordinator",
                run_id: runId,
                step_key: "coordinator_plan",
                member_id: longCoordinatorId,
                remote_task_id: `remote-${"r".repeat(64)}`,
                status: "working",
                attempt: 1,
                depends_on: [],
                input: {},
                output: null,
                error_text: null,
                started_at: fixture.now + 11,
                ended_at: null,
              },
              session_status: "working",
            },
            {
              member_id: longWorkerId1,
              role: "worker",
              model: "gemini",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"a".repeat(52)}`],
              pending_inbox_count: 0,
              status: "submitted",
              latest_step: null,
              session_status: "idle",
            },
            {
              member_id: longWorkerId2,
              role: "worker",
              model: "kimi",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"b".repeat(52)}`],
              pending_inbox_count: 0,
              status: "submitted",
              latest_step: null,
              session_status: "idle",
            },
          ],
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
    }
  );

  await page.route(new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.setViewportSize({ width: 1366, height: 900 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Desktop");
  await openMainTeamAction(page, "Execution Runs");
  await expect(page.locator(".teams-main").getByText("Team Desktop", { exact: true })).toBeVisible();
  await openAdvancedView(page, "Overview");
  await expect(page.locator(".teams-member-list .team-member-row")).toHaveCount(3);
  await expect(page.locator(".teams-overview-meta")).toBeVisible();

  const overviewLayout = await page.evaluate(() => {
    const selectors = [".teams-overview-meta", ".teams-member-list"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(overviewLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(overviewLayout.overflowing).toEqual([]);

  await selectAgentFromSidebar(page, longCoordinatorId);
  await openAdvancedView(page, "Member Console");
  const memberConsoleCard = page.locator('[data-team-panel="member-console"]');
  await expect(memberConsoleCard).toBeVisible();
  await memberConsoleCard.locator("select").first().selectOption(longCoordinatorId);
  await expect(memberConsoleCard).toContainText("mcp_skills");
  await memberConsoleCard.locator("summary", { hasText: "prompt" }).click();

  const memberConsoleLayout = await page.evaluate(() => {
    const selectors = [".teams-step-body"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(memberConsoleLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(memberConsoleLayout.overflowing).toEqual([]);

  await selectPrimaryTeamEntryFromSidebar(page, "all");
  await expect(page.getByRole("heading", { name: "# all", exact: true })).toBeVisible();
  await openAdvancedView(page, "Execution Mailbox");
  await expect(page.locator(".teams-chat-head")).toBeVisible();
  const mailboxLayout = await page.evaluate(() => {
    const selectors = [".teams-chat-head"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(mailboxLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(mailboxLayout.overflowing).toEqual([]);
});

test("team setup keeps add agent wording after the first member binds", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  fixture.agents.splice(0, fixture.agents.length);
  const teamName = "forge-team";

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: teamName,
    goal: "Bind coordinator in-place before worker setup.",
  });
  await expectAddAgentEntryVisible(page, teamName);
  await expect(page.getByText("No agents have joined this team yet.")).toBeVisible();

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/forge-coordinator",
    identity: "Coordinator bound in-place",
  });

  await openTeamFromSelector(page, teamName);
  await expectAddAgentEntryVisible(page, teamName);
  const updates = fixture.getUpdateSpecPayloads();
  expect(updates).toHaveLength(1);
  expect(updates[0]?.payload.spec.members[0]?.member_id).toBe("agent-forge-1");
});

test("team quant workflow creates team and launches run", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const runsByTeamId = new Map<string, TeamRunRecord[]>();
  const runById = new Map<string, TeamRunRecord>();
  const runStepsById = new Map<string, Array<Record<string, unknown>>>();
  const runEventsById = new Map<string, Array<Record<string, unknown>>>();
  const nextRunIndexByTeamId = new Map<string, number>();

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/)?.[1] ?? "";
    if (!teamId) {
      await route.fallback();
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        context_id?: string;
        input?: Record<string, unknown>;
      };
      const nextIndex = (nextRunIndexByTeamId.get(teamId) ?? 0) + 1;
      nextRunIndexByTeamId.set(teamId, nextIndex);
      const createdAt = fixture.now + 1_000 + nextIndex;
      const run: TeamRunRecord = {
        id: `${teamId}-quant-run-${nextIndex}`,
        team_id: teamId,
        context_id: payload.context_id ?? `ctx-${teamId}-${nextIndex}`,
        status: "working",
        input: payload.input ?? {},
        created_at: createdAt,
        started_at: createdAt + 1,
        ended_at: null,
      };
      const prev = runsByTeamId.get(teamId) ?? [];
      runsByTeamId.set(teamId, [run, ...prev]);
      runById.set(run.id, run);
      runStepsById.set(run.id, [
        {
          id: `${run.id}-step-1`,
          run_id: run.id,
          step_key: "coordinator_plan",
          member_id: "quant-coordinator",
          remote_task_id: "task-coordinator-plan",
          status: "working",
          attempt: 1,
          depends_on: [],
          input: run.input,
          output: null,
          error_text: null,
          started_at: createdAt + 2,
          ended_at: null,
        },
        {
          id: `${run.id}-step-2`,
          run_id: run.id,
          step_key: "worker_portfolio_optimize",
          member_id: "portfolio-worker",
          remote_task_id: null,
          status: "submitted",
          attempt: 1,
          depends_on: ["coordinator_plan"],
          input: {},
          output: null,
          error_text: null,
          started_at: null,
          ended_at: null,
        },
        {
          id: `${run.id}-step-3`,
          run_id: run.id,
          step_key: "worker_crypto_algo_trade",
          member_id: "crypto-worker",
          remote_task_id: null,
          status: "submitted",
          attempt: 1,
          depends_on: ["coordinator_plan"],
          input: {},
          output: null,
          error_text: null,
          started_at: null,
          ended_at: null,
        },
      ]);
      runEventsById.set(run.id, [
        {
          event_id: 1,
          run_id: run.id,
          step_id: null,
          event_type: "run_submitted",
          ts: createdAt,
          payload: { status: "submitted" },
        },
        {
          event_id: 2,
          run_id: run.id,
          step_id: `${run.id}-step-1`,
          event_type: "run_working",
          ts: createdAt + 1,
          payload: { status: "working" },
        },
      ]);
      await route.fulfill(jsonResponse(run));
      return;
    }
    if (request.method() === "GET") {
      const status = url.searchParams.get("status");
      const beforeCreatedAtRaw = url.searchParams.get("before_created_at");
      const beforeCreatedAt =
        beforeCreatedAtRaw == null ? null : Number(beforeCreatedAtRaw);
      const limitRaw = Number(url.searchParams.get("limit") ?? "50");
      const limit = Number.isFinite(limitRaw) && limitRaw > 0 ? limitRaw : 50;
      const base = runsByTeamId.get(teamId) ?? [];
      const filtered = base
        .filter((run) => (status && status !== "all" ? run.status === status : true))
        .filter((run) =>
          beforeCreatedAt == null ? true : run.created_at < beforeCreatedAt
        );
      await route.fulfill(jsonResponse(filtered.slice(0, limit)));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/?]+)/)?.[1] ?? "";
    const run = runById.get(runId);
    if (!run) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(run));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/steps$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/steps$/)?.[1] ?? "";
    await route.fulfill(jsonResponse(runStepsById.get(runId) ?? []));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/events/ )?.[1] ?? "";
    await route.fulfill(jsonResponse(runEventsById.get(runId) ?? []));
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
      const run = runById.get(runId);
      if (!run) {
        await route.fulfill(jsonResponse({ error: "run not found" }, 404));
        return;
      }
      const team = fixture.teams.find((item) => item.id === run.team_id);
      if (!team) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      const members = (team.spec.members ?? []).map((member, index) => ({
        member_id: member.member_id,
        role: member.role ?? "worker",
        model: member.model ?? null,
        prompt: null,
        skills: member.skills ?? [],
        pending_inbox_count: 0,
        status: index === 0 ? "working" : "submitted",
        latest_step: null,
        session_status: index === 0 ? "working" : "idle",
      }));
      await route.fulfill(
        jsonResponse({
          run,
          team,
          coordinator_member_id: team.spec.coordinator_member_id,
          members,
          steps: runStepsById.get(run.id) ?? [],
          latest_events: runEventsById.get(run.id) ?? [],
          mailbox: {
            pending: 0,
            delivered: 0,
            dead_letter: 0,
            recent_messages: [],
          },
        })
      );
    }
  );

  const quantSpec = {
    spec_version: 1,
    entrypoint: "coordinator_plan",
    coordinator_member_id: "quant-coordinator",
    members: [
      {
        member_id: "quant-coordinator",
        role: "coordinator",
        model: "codex",
        prompt: "Own run-level planning, risk budget, and compute/resource control.",
        skills: ["agenthub-actor-runtime", "team-coordinator-orchestrator"],
      },
      {
        member_id: "portfolio-worker",
        role: "worker",
        model: "gemini",
        prompt: "Do portfolio optimization with risk-parity and exposure constraints.",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
      },
      {
        member_id: "crypto-worker",
        role: "worker",
        model: "kimi",
        prompt: "Run crypto algo trading simulation and report pnl/drawdown.",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
      },
    ],
    steps: [
      { step_key: "coordinator_plan", member_id: "quant-coordinator", depends_on: [] },
      {
        step_key: "worker_portfolio_optimize",
        member_id: "portfolio-worker",
        depends_on: ["coordinator_plan"],
      },
      {
        step_key: "worker_crypto_algo_trade",
        member_id: "crypto-worker",
        depends_on: ["coordinator_plan"],
      },
      {
        step_key: "coordinator_synthesize",
        member_id: "quant-coordinator",
        depends_on: ["worker_portfolio_optimize", "worker_crypto_algo_trade"],
      },
    ],
  };

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: "quant-alpha-desk",
    goal: "coordinator manages resources; workers optimize portfolio + crypto trading",
  });
  fixture.agents.push(
    {
      id: "quant-coordinator",
      name: "quant-coordinator",
      workdir: "/workspace/quant-coordinator",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 200,
      updated_at: fixture.now + 200,
    },
    {
      id: "portfolio-worker",
      name: "portfolio-worker",
      workdir: "/workspace/portfolio-worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 201,
      updated_at: fixture.now + 201,
    },
    {
      id: "crypto-worker",
      name: "crypto-worker",
      workdir: "/workspace/crypto-worker",
      command: "kimi",
      args: ["acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 202,
      updated_at: fixture.now + 202,
    }
  );
  const createdTeam = fixture.teams.find((team) => team.name === "quant-alpha-desk");
  if (!createdTeam) {
    throw new Error("quant-alpha-desk was not created");
  }
  createdTeam.spec = quantSpec;
  createdTeam.updated_at += 1;
  await gotoTeams(page);
  await openTeamFromSelector(page, "quant-alpha-desk");

  const createPayload = fixture.getCreatePayload();
  expect(createPayload).not.toBeNull();
  expect((createPayload as CreateTeamPayload).spec).toEqual({
    spec_version: 1,
    members: [],
  });

  await openAdvancedView(page, "Debug");
  await page
    .getByPlaceholder("context_id (optional, auto-generated when empty)")
    .fill("quant-run-ctx");
  await page
    .getByLabel("Run input JSON")
    .fill('{"objective":"daily rebalance + crypto hedge","risk_limit":"max_dd_5pct"}');
  await page.getByRole("button", { name: "Create Run", exact: true }).click();

  await openMainTeamAction(page, "Execution Runs");
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "quant-run-1"
  );
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "working"
  );
  await openAdvancedView(page, "Overview");
  await expect(page.locator(".team-member-row", { hasText: "quant-coordinator" })).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "portfolio-worker" })
  ).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "crypto-worker" })
  ).toBeVisible();
});
