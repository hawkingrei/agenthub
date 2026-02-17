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
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(agents));
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
  await expect(page.getByRole("button", { name: "Create Team" })).toBeVisible();
  await page.getByRole("button", { name: "Create Team" }).click();

  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await expect(dialog).toBeVisible();
  await dialog.getByPlaceholder("team name").fill("quest-team");
  await dialog.getByPlaceholder("description (optional)").fill("team from e2e");
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  const leaderPanel = dialog.locator(".team-create-panel");
  await leaderPanel.locator("select").first().selectOption("agent-leader-1");
  await leaderPanel.locator("select").nth(1).selectOption("codex");
  await expect(leaderPanel.getByText("workdir: /workspace/leader")).toBeVisible();
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Recruit Workers" })).toBeVisible();
  const workerCard = dialog.locator(".teams-worker-card").first();
  await workerCard.locator("select").first().selectOption("agent-worker-1");
  await workerCard.locator("select").nth(1).selectOption("gemini");
  await expect(workerCard.getByText("workdir: /workspace/worker")).toBeVisible();
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Launch Team" })).toBeVisible();
  const specEditor = dialog.locator(".team-create-panel textarea");
  await expect(specEditor).toContainText("\"leader_member_id\": \"agent-leader-1\"");
  await expect(specEditor).toContainText("\"step_key\": \"leader_plan\"");
  await expect(specEditor).toContainText("\"step_key\": \"leader_synthesize\"");

  await dialog.getByRole("button", { name: "Create Team" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".team-item", { hasText: "quest-team" })).toBeVisible();

  const payload = fixture.getCreatePayload();
  expect(payload).not.toBeNull();
  const createdPayload = payload as CreateTeamPayload;
  expect(createdPayload.name).toBe("quest-team");
  expect(createdPayload.spec.leader_member_id).toBe("agent-leader-1");
  const leaderMember = createdPayload.spec.members.find(
    (member) => member.role === "leader"
  );
  const workerMember = createdPayload.spec.members.find(
    (member) => member.role === "worker"
  );
  expect(leaderMember?.model).toBe("codex");
  expect(workerMember?.model).toBe("gemini");
  expect(
    createdPayload.spec.steps.some((step) => step.step_key === "leader_plan")
  ).toBe(true);
  expect(
    createdPayload.spec.steps.some(
      (step) => step.step_key === "leader_synthesize"
    )
  ).toBe(true);
});

test("team forge blocks stage advance when duplicate assignments exist", async ({
  page,
}) => {
  await mockTeamPageApis(page);
  await page.goto("/teams");

  await page.getByRole("button", { name: "Create Team" }).click();
  const dialog = page.getByRole("dialog", { name: "Team Forge" });
  await dialog.getByPlaceholder("team name").fill("dup-team");
  await dialog.getByRole("button", { name: "Next Stage" }).click();

  await expect(dialog.getByRole("heading", { name: "Leader Forge" })).toBeVisible();
  const leaderPanel = dialog.locator(".team-create-panel");
  await leaderPanel.locator("select").first().selectOption("agent-worker-1");
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
  await expect(page.getByRole("button", { name: "Load More" })).toBeEnabled();
  await page.getByRole("button", { name: "Load More" }).click();

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
