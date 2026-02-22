import { expect, test } from "./coverage";

type StoredAuthState = {
  token: string;
  userId: string;
  username: string;
  role: string;
};

type E2eAgentRecord = {
  id: string;
  name: string;
  workdir: string;
  command: string;
  args: string[];
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  status: string;
  created_at: number;
  updated_at: number;
};

type TeamSpecMember = {
  member_id: string;
  role?: string;
  model?: string;
  skills?: string[];
};

type TeamSpecStep = {
  step_key: string;
};

type TeamSpecPayload = {
  leader_member_id?: string;
  members: TeamSpecMember[];
  steps: TeamSpecStep[];
};

type CreateTeamPayload = {
  name: string;
  description?: string;
  spec: TeamSpecPayload;
};

type TeamDefinitionRecord = {
  id: string;
  name: string;
  description?: string | null;
  spec: TeamSpecPayload;
  created_at: number;
  updated_at: number;
};

type TeamRunRecord = {
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

type TeamActorMessageRecord = {
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

type TeamPageFixture = {
  now: number;
  auth: StoredAuthState;
  agents: E2eAgentRecord[];
  teams: TeamDefinitionRecord[];
  getCreatePayload: () => CreateTeamPayload | null;
};

function jsonResponse(data: unknown, status = 200): {
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

function buildTeamRun(
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

async function createForgeAgentFromModal(
  page: import("@playwright/test").Page,
  workdir: string
): Promise<void> {
  const forgeDialog = page
    .locator(".mantine-Modal-content")
    .filter({ hasText: "Create Agent" })
    .last();
  await expect(forgeDialog).toBeVisible();
  await forgeDialog.getByLabel(/Workdir/).fill(workdir);
  await forgeDialog.getByRole("button", { name: "Create Agent" }).click();
  await expect(forgeDialog).toBeHidden();
}

async function mockTeamPageApis(
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
      id: "agent-leader-1",
      name: "Leader Agent",
      workdir: "/workspace/leader",
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
      args: ["--experimental-acp"],
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
  const teams: TeamDefinitionRecord[] = [];
  let createTeamPayload: CreateTeamPayload | null = null;

  await page.addInitScript((storedAuth: StoredAuthState) => {
    window.localStorage.setItem("agenthub_auth", JSON.stringify(storedAuth));
  }, auth);

  await page.route("**/api/auth/status", async (route) => {
    await route.fulfill(jsonResponse({ root_initialized: true }));
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
      await route.fulfill(jsonResponse(deleted));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  return {
    now,
    auth,
    agents,
    teams,
    getCreatePayload: () => createTeamPayload,
  };
}

test("team forge modal creates team with leader/worker presets", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);

  await page.goto("/teams");

  await expect(page.getByRole("heading", { name: "AgentHub Teams" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Guided Wizard" })).toBeVisible();
  await page.getByRole("button", { name: "Guided Wizard" }).click();

  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await expect(dialog).toBeVisible();
  await dialog.getByPlaceholder("team name").fill("quest-team");
  await dialog.getByPlaceholder("description (optional)").fill("team from e2e");
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  const leaderPanel = dialog.locator(".team-create-panel");
  await dialog.getByRole("button", { name: "New Agent" }).click();
  await createForgeAgentFromModal(page, "/workspace/leader");
  await expect(leaderPanel.locator("select").first()).toHaveValue(/agent-forge-/);
  await leaderPanel.locator("select").nth(1).selectOption("codex");
  await leaderPanel
    .getByPlaceholder("leader custom skills (comma separated, optional)")
    .fill("custom-leader-skill");
  await expect(leaderPanel.getByText("workdir: /workspace/leader")).toBeVisible();
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Recruit Workers" })).toBeVisible();
  await dialog.getByRole("button", { name: "New Agent" }).click();
  await createForgeAgentFromModal(page, "/workspace/worker");
  const workerCard = dialog.locator(".teams-worker-card").first();
  await expect(workerCard.locator("select").first()).toHaveValue(/agent-forge-/);
  await workerCard.locator("select").nth(1).selectOption("gemini");
  await workerCard
    .getByPlaceholder("worker custom skills (comma separated, optional)")
    .fill("custom-worker-skill");
  await expect(workerCard.getByText("workdir: /workspace/worker")).toBeVisible();
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Launch Team" })).toBeVisible();
  const specEditor = dialog.locator(".team-create-panel textarea");
  await expect(specEditor).toContainText("\"leader_member_id\": \"agent-forge-");
  await expect(specEditor).toContainText("\"step_key\": \"leader_plan\"");
  await expect(specEditor).toContainText("\"step_key\": \"leader_synthesize\"");

  await dialog.getByRole("button", { name: "Create Team" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".team-item", { hasText: "quest-team" })).toBeVisible();

  const payload = fixture.getCreatePayload();
  expect(payload).not.toBeNull();
  const createdPayload = payload as CreateTeamPayload;
  expect(createdPayload.name).toBe("quest-team");
  expect(createdPayload.spec.leader_member_id).toMatch(/^agent-forge-/);
  const leaderMember = createdPayload.spec.members.find(
    (member) => member.role === "leader"
  );
  const workerMember = createdPayload.spec.members.find(
    (member) => member.role === "worker"
  );
  expect(leaderMember?.member_id).toMatch(/^agent-forge-/);
  expect(workerMember?.member_id).toMatch(/^agent-forge-/);
  expect(leaderMember?.model).toBe("codex");
  expect(workerMember?.model).toBe("gemini");
  expect(leaderMember?.skills).toContain("agenthub-actor-runtime");
  expect(leaderMember?.skills).toContain("custom-leader-skill");
  expect(workerMember?.skills).toContain("agenthub-actor-runtime");
  expect(workerMember?.skills).toContain("custom-worker-skill");
  expect(
    createdPayload.spec.steps.some((step) => step.step_key === "leader_plan")
  ).toBe(true);
  expect(
    createdPayload.spec.steps.some(
      (step) => step.step_key === "leader_synthesize"
    )
  ).toBe(true);
});

test("team forge manual spec mode skips leader/worker stages", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);

  await page.goto("/teams");
  await page.getByRole("button", { name: "Manual Spec" }).click();

  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await dialog.getByPlaceholder("team name").fill("manual-spec-team");
  await dialog.getByRole("button", { name: "Next Stage" }).click();
  await expect(dialog.getByRole("heading", { name: "Launch Team" })).toBeVisible();
  await expect(dialog.getByText("Stage 4/4")).toBeVisible();

  const customSpec = {
    spec_version: 1,
    entrypoint: "leader_plan",
    leader_member_id: "leader-manual",
    members: [{ member_id: "leader-manual", role: "leader", model: "codex" }],
    steps: [{ step_key: "leader_plan", member_id: "leader-manual", depends_on: [] }],
  };

  await dialog
    .locator(".team-create-panel textarea")
    .fill(JSON.stringify(customSpec, null, 2));
  await dialog.getByRole("button", { name: "Create Team" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".team-item", { hasText: "manual-spec-team" })).toBeVisible();

  const payload = fixture.getCreatePayload();
  expect(payload).not.toBeNull();
  const createdPayload = payload as CreateTeamPayload;
  expect(createdPayload.name).toBe("manual-spec-team");
  expect(createdPayload.spec).toEqual(customSpec);
});

test("team forge blocks stage advance when duplicate assignments exist", async ({
  page,
}) => {
  await mockTeamPageApis(page);
  await page.goto("/teams");

  await page.getByRole("button", { name: "Guided Wizard" }).click();
  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await dialog.getByPlaceholder("team name").fill("dup-team");
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  const leaderPanel = dialog.locator(".team-create-panel");
  await dialog.getByRole("button", { name: "New Agent" }).click();
  await createForgeAgentFromModal(page, "/workspace/dup-a");
  await dialog.getByRole("button", { name: "New Agent" }).click();
  await createForgeAgentFromModal(page, "/workspace/dup-b");
  const leaderSelect = leaderPanel.locator("select").first();
  await leaderSelect.selectOption({ index: 1 });
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Recruit Workers" })).toBeVisible();
  await dialog.getByRole("button", { name: "Add Worker" }).click();
  const workerCard = dialog.locator(".teams-worker-card").first();
  const workerSelect = workerCard.locator("select").first();
  await workerSelect.selectOption({ index: 1 });
  const workerMemberId = await workerSelect.inputValue();
  await expect(dialog.getByText("Duplicate assignments detected:")).toHaveCount(0);

  await dialog.getByRole("button", { name: "Back" }).click();
  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  await leaderSelect.selectOption(workerMemberId);
  await dialog.getByRole("button", { name: "Next Stage" }).click();
  await expect(dialog.getByRole("heading", { name: "Recruit Workers" })).toBeVisible();

  await expect(dialog.getByText("Duplicate assignments detected:")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Next Stage" })).toBeDisabled();
  await dialog.getByRole("button", { name: "Resolve Duplicates" }).click();
  await expect(dialog.getByText("Duplicate assignments detected:")).toHaveCount(0);
  await expect(dialog.getByRole("button", { name: "Next Stage" })).toBeEnabled();
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Launch Team" })).toBeVisible();
});

test("team page keeps single-column proportions on mobile viewport", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longLeaderId = `agent-leader-${"x".repeat(72)}`;
  const longWorkerId = `agent-worker-${"y".repeat(72)}`;
  fixture.teams.push({
    id: "team-mobile",
    name: "Team Mobile",
    description: "mobile layout regression guard",
    spec: {
      leader_member_id: longLeaderId,
      members: [
        { member_id: longLeaderId, role: "leader", model: "codex" },
        { member_id: longWorkerId, role: "worker", model: "gemini" },
      ],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/teams");

  await expect(page.getByRole("heading", { name: "Team Mobile" })).toBeVisible();

  const layoutColumns = await page.locator(".teams-layout").evaluate((element) => {
    return window.getComputedStyle(element).gridTemplateColumns;
  });
  expect(layoutColumns.trim().split(/\s+/).length).toBe(1);

  const runFilterWidth = await page
    .locator(".teams-run-list-head .actions select")
    .first()
    .evaluate((element) => {
      return element.getBoundingClientRect().width;
    });
  expect(runFilterWidth).toBeGreaterThan(240);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("team page desktop keeps long metadata blocks non-overlapping", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longLeaderId = `leader-${"l".repeat(96)}`;
  const longWorkerId1 = `worker-a-${"w".repeat(88)}`;
  const longWorkerId2 = `worker-b-${"z".repeat(88)}`;
  const longPrompt = `prompt-${"p".repeat(420)}`;
  const teamId = "team-desktop";
  fixture.teams.push({
    id: teamId,
    name: "Team Desktop",
    description: "desktop overlap regression guard",
    spec: {
      leader_member_id: longLeaderId,
      members: [
        {
          member_id: longLeaderId,
          role: "leader",
          model: "codex",
          skills: ["agenthub-actor-runtime", "team-leader-orchestrator", `mcp-${"m".repeat(52)}`],
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
      steps: [{ step_key: "leader_plan" }],
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
          leader_member_id: longLeaderId,
          members: [
            {
              member_id: longLeaderId,
              role: "leader",
              model: "codex",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-leader-orchestrator", `mcp-${"m".repeat(52)}`],
              pending_inbox_count: 0,
              status: "working",
              latest_step: {
                id: "step-leader",
                run_id: runId,
                step_key: "leader_plan",
                member_id: longLeaderId,
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
  await page.goto("/teams");
  await expect(page.getByRole("heading", { name: "Team Desktop" })).toBeVisible();
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

  await page.getByRole("button", { name: "Member Console" }).click();
  const memberConsoleCard = page.locator(".card", { hasText: "Member Console" });
  await expect(memberConsoleCard).toBeVisible();
  await memberConsoleCard.locator("select").first().selectOption(longLeaderId);
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

  await page.getByRole("button", { name: "Mailbox", exact: true }).click();
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

test("team forge agent entry creates and binds leader in-place", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  fixture.agents.splice(0, fixture.agents.length);

  await page.goto("/teams");
  await page.getByRole("button", { name: "Guided Wizard" }).click();
  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await dialog.getByPlaceholder("team name").fill("forge-team");
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  await expect(
    dialog.getByText("No forged agents yet. Create one in the Agent Forge entry above.")
  ).toBeVisible();
  await dialog.getByRole("button", { name: "New Agent" }).click();
  await createForgeAgentFromModal(page, "/workspace/forge-leader");

  await expect(dialog.getByText("agent_id: agent-forge-1")).toBeVisible();
  await expect(dialog.getByText("workdir: /workspace/forge-leader")).toBeVisible();
});

test("team quant workflow creates team and launches run", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
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
          step_key: "leader_plan",
          member_id: "quant-leader",
          remote_task_id: "task-leader-plan",
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
          depends_on: ["leader_plan"],
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
          depends_on: ["leader_plan"],
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
          leader_member_id: team.spec.leader_member_id,
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
    entrypoint: "leader_plan",
    leader_member_id: "quant-leader",
    members: [
      {
        member_id: "quant-leader",
        role: "leader",
        model: "codex",
        prompt: "Own run-level planning, risk budget, and compute/resource control.",
        skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
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
      { step_key: "leader_plan", member_id: "quant-leader", depends_on: [] },
      {
        step_key: "worker_portfolio_optimize",
        member_id: "portfolio-worker",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "worker_crypto_algo_trade",
        member_id: "crypto-worker",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "leader_synthesize",
        member_id: "quant-leader",
        depends_on: ["worker_portfolio_optimize", "worker_crypto_algo_trade"],
      },
    ],
  };

  await page.goto("/teams");
  await page.getByRole("button", { name: "Manual Spec" }).click();

  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await dialog.getByPlaceholder("team name").fill("quant-alpha-desk");
  await dialog
    .getByPlaceholder("description (optional)")
    .fill("leader manages resources; workers optimize portfolio + crypto trading");
  await dialog.getByRole("button", { name: "Next Stage" }).click();
  await expect(dialog.getByRole("heading", { name: "Launch Team" })).toBeVisible();

  await dialog
    .locator(".team-create-panel textarea")
    .fill(JSON.stringify(quantSpec, null, 2));
  await dialog.getByRole("button", { name: "Create Team" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".team-item", { hasText: "quant-alpha-desk" })).toBeVisible();

  const createPayload = fixture.getCreatePayload();
  expect(createPayload).not.toBeNull();
  expect((createPayload as CreateTeamPayload).spec).toEqual(quantSpec);

  await page
    .getByPlaceholder("context_id (optional, auto-generated when empty)")
    .fill("quant-run-ctx");
  await page
    .getByLabel("Run input JSON")
    .fill('{"objective":"daily rebalance + crypto hedge","risk_limit":"max_dd_5pct"}');
  await page.getByRole("button", { name: "Create Run", exact: true }).click();

  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "quant-run-1"
  );
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "working"
  );
  await expect(page.locator(".team-member-row", { hasText: "quant-leader" })).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "portfolio-worker" })
  ).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "crypto-worker" })
  ).toBeVisible();
});

test("team debug run ops compiles main task preview and applies payload to create-run form", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-compile";
  const teamCreatedAt = fixture.now + 120;
  const previewResponse = {
    main_task_id: "main-task-compile-1",
    conversation_id: "conversation-compile-1",
    run_payload: {
      context_id: "ctx-main-task-compile-1",
      input: {
        main_task_compile_version: 1,
        main_task_id: "main-task-compile-1",
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
    new RegExp(`/api/teams/${teamId}/main_tasks/[^/]+/compile_run_preview$`),
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

  await page.goto("/teams");
  await expect(page.getByRole("heading", { name: "Debug Run Ops", exact: true })).toBeVisible();

  await page.getByPlaceholder("main_task_id").fill("main-task-compile-1");
  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();

  await expect(page.getByText("main_task_id: main-task-compile-1")).toBeVisible();
  await expect(page.getByText("conversation_id: conversation-compile-1")).toBeVisible();
  await expect(page.getByText("context_id: ctx-main-task-compile-1")).toBeVisible();
  expect(compileRequests).toEqual([{}]);

  await page.getByRole("button", { name: "Use Payload in Create Run" }).click();
  await expect(
    page.getByPlaceholder("context_id (optional, auto-generated when empty)")
  ).toHaveValue("ctx-main-task-compile-1");
  await expect(page.getByLabel("Run input JSON")).toContainText(
    '"main_task_id": "main-task-compile-1"'
  );
});

test("team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-chat-first";
  const runId = "run-chat-first-1";
  const teamCreatedAt = fixture.now + 180;
  const runCreatedAt = fixture.now + 260;
  const previewResponse = {
    main_task_id: "main-task-chat-1",
    conversation_id: "conversation-chat-1",
    run_payload: {
      context_id: "ctx-chat-first-1",
      input: {
        main_task_compile_version: 1,
        main_task_id: "main-task-chat-1",
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
  const sentMessagePayloads: Array<{
    from_actor_id: string;
    to_actor_id: string;
    payload: unknown;
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
    new RegExp(`/api/teams/${teamId}/main_tasks/[^/]+/compile_run_preview$`),
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

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.goto("/teams");
  await expect(page.getByRole("heading", { name: "Chat First Team" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Debug Run Ops", exact: true })).toBeVisible();

  await page.getByPlaceholder("main_task_id").fill("main-task-chat-1");
  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect(page.getByText("conversation_id: conversation-chat-1")).toBeVisible();
  await expect(page.getByText("Negotiate scope with leader")).toBeVisible();

  await page.getByRole("button", { name: "Create Run from Preview" }).click();
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(runId);
  expect(createRunRequests).toHaveLength(1);
  expect(createRunRequests[0]).toMatchObject({
    context_id: "ctx-chat-first-1",
  });
  expect(
    (createRunRequests[0]?.input as { main_task_id?: string } | undefined)
      ?.main_task_id
  ).toBe("main-task-chat-1");

  await page.getByRole("button", { name: "Mailbox", exact: true }).click();
  await page
    .locator(".teams-chat-members .team-item", { hasText: "agent-worker-1 (worker)" })
    .click();
  await expect(page.locator(".teams-chat-messages")).toContainText(
    "Please implement endpoint scaffolding and tests."
  );
  await expect(page.locator(".teams-chat-messages")).toContainText(
    "Endpoint and tests are complete."
  );

  await page
    .getByPlaceholder("Type a message to selected agent")
    .fill("Please include migration notes in the final report.");
  await page.getByRole("button", { name: "Send Chat" }).click();
  await expect(page.locator(".teams-chat-messages")).toContainText(
    "Please include migration notes in the final report."
  );
  expect(sentMessagePayloads).toHaveLength(1);
  expect(sentMessagePayloads[0]).toMatchObject({
    from_actor_id: "agent-leader-1",
    to_actor_id: "agent-worker-1",
  });

  await page.getByRole("button", { name: "Member Console" }).click();
  await expect(page.locator(".teams-step-body")).toContainText("a2a_discovery_card");
  await expect(page.locator(".teams-step-body")).not.toContainText("Loading discovery card...");
  await expect(page.locator(".teams-step-body")).toContainText("acp_gemini");

  await page.getByRole("button", { name: "Events" }).click();
  await expect(page.locator(".teams-event-list")).toContainText(
    "Final deliverable prepared and returned to user."
  );
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
    const label = await page
      .locator(".teams-chat-members .team-item", { hasText: `${memberId} (` })
      .locator(".teams-member-unread")
      .innerText();
    const match = label.match(/unread=(\d+)/);
    return match ? Number(match[1]) : 0;
  };

  await page.goto("/teams");
  await expect(page.getByRole("heading", { name: "Team Mailbox" })).toBeVisible();

  await page.locator(".tab", { hasText: "Mailbox" }).click();
  await expect(page.locator(".teams-chat-shell")).toBeVisible();

  const unreadWorker1Before = await unreadFor("agent-worker-1");
  expect(unreadWorker1Before).toBeGreaterThan(0);
  const unreadWorker2Before = await unreadFor("agent-worker-2");
  expect(unreadWorker2Before).toBeGreaterThan(0);

  await page
    .locator(".teams-chat-members .team-item", { hasText: "agent-worker-1 (worker)" })
    .click();
  await expect(page.locator(".teams-chat-head")).toContainText(
    "agent-leader-1 → agent-worker-1"
  );
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=on");
  await expect.poll(async () => unreadFor("agent-worker-1")).toBe(0);
  expect(await unreadFor("agent-worker-2")).toBeGreaterThan(0);

  await page.locator(".teams-chat-messages").evaluate((element) => {
    const target = element as HTMLElement;
    target.scrollTop = 0;
    target.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=off");

  await page.getByRole("button", { name: "Overview" }).click();
  await page
    .locator(".teams-member-list .team-item", { hasText: "agent-worker-2 (worker)" })
    .click();
  await expect(page.locator(".tab.active")).toContainText("Mailbox");
  await expect(page.locator(".teams-chat-head")).toContainText(
    "agent-leader-1 → agent-worker-2"
  );

  const eventsBeforePolling = counters.events;
  const snapshotBeforePolling = counters.snapshot;
  const inboxBeforePolling = counters.inbox;
  await page.waitForTimeout(4500);
  expect(counters.events).toBe(eventsBeforePolling);
  expect(counters.snapshot).toBeGreaterThan(snapshotBeforePolling);
  expect(counters.inbox).toBeGreaterThan(inboxBeforePolling);

  await page.getByRole("button", { name: "Debug" }).click();
  await page.getByRole("button", { name: "Mailbox Raw" }).click();
  await expect(page.getByRole("heading", { name: "Send Message (JSON)" })).toBeVisible();

  const advancedPanel = page.locator(".teams-message-advanced .teams-message-panel").first();
  await advancedPanel.getByPlaceholder("from_actor_id").fill("agent-leader-1");
  await advancedPanel.getByPlaceholder("to_actor_id").fill("agent-worker-2");
  await advancedPanel
    .getByPlaceholder("payload JSON")
    .fill('{"type":"chat_message","text":"advanced-mailbox-ping"}');
  await advancedPanel.getByRole("button", { name: "Send Message" }).click();

  await page.getByRole("button", { name: "Mailbox", exact: true }).click();
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

  await page.goto("/teams");
  await expect(page.locator(".teams-sidebar .team-item", { hasText: "Team Delete A" })).toBeVisible();
  await expect(page.locator(".teams-sidebar .team-item", { hasText: "Team Delete B" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Team Delete A" })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete Team" }).click();

  await expect(page.locator(".teams-sidebar .team-item", { hasText: "Team Delete A" })).toHaveCount(0);
  await expect(page.locator(".teams-sidebar .team-item", { hasText: "Team Delete B" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Team Delete B" })).toBeVisible();
});

test("team run list keeps per-team filters and uses before_created_at cursor paging", async ({
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

  await page.goto("/teams");

  const runFilter = page.getByLabel("Run status filter");
  await expect(runFilter).toHaveValue("all");

  await runFilter.selectOption("working");
  await expect(runFilter).toHaveValue("working");
  const loadMoreRunsButton = page.getByRole("button", { name: "Load More" });
  await expect(loadMoreRunsButton).toBeEnabled();
  await loadMoreRunsButton.focus();
  await loadMoreRunsButton.press("Enter");

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

  const teamAItem = page.locator(".teams-sidebar .teams-list .team-item", {
    hasText: "Team A",
  });
  const teamBItem = page.locator(".teams-sidebar .teams-list .team-item", {
    hasText: "Team B",
  });

  await teamBItem.click();
  await expect(runFilter).toHaveValue("all");
  await runFilter.selectOption("failed");
  await expect(runFilter).toHaveValue("failed");

  await teamAItem.click();
  await expect(runFilter).toHaveValue("working");

  await teamBItem.click();
  await expect(runFilter).toHaveValue("failed");
});
