import { expect, test } from "@playwright/test";

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
