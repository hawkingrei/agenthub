import { expect, test } from "./coverage";
import {
  createTeamFromModal,
  gotoTeams,
  jsonResponse,
  mockTeamPageApis,
  openMainTeamAction,
  openTeamFromSelector,
  selectedTeamMenuLocator,
} from "./team_page_helpers";

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

  const titleBox = await page.locator('[data-workspace-shell-primary="true"]').boundingBox();
  const actionsBox = await page.locator('[data-workspace-shell-actions="true"]').boundingBox();
  expect(titleBox).not.toBeNull();
  expect(actionsBox).not.toBeNull();
  await expect(page.locator('[data-workspace-shell-lenses="true"]')).toHaveCount(0);

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

test("team channel image upload stays hidden when mobile composer is unavailable", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push({
    id: "team-mobile-image-upload",
    name: "Mobile Image Upload Team",
    description: "mobile graph-bed image upload e2e",
    spec: {
      coordinator_member_id: "agent-coordinator-1",
      members: [{ member_id: "agent-coordinator-1", role: "coordinator", model: "codex" }],
      steps: [{ step_key: "coordinator_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Mobile Image Upload Team");
  await expect(page.getByRole("button", { name: "Upload image", exact: true })).toHaveCount(0);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("node detail keeps mobile detail surfaces stacked without horizontal overflow", async ({
  page,
}) => {
  await mockTeamPageApis(page);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/workspace/nodes", { waitUntil: "domcontentloaded" });

  await expect(page.getByText("Node Detail", { exact: true })).toBeVisible();
  await expect(page.getByText("Observed Agent Runtimes", { exact: true })).toBeVisible();
  await expect(page.getByText("Connect Command", { exact: true }).first()).toBeVisible();

  const detailLayoutColumns = await page
    .locator('[data-node-detail-layout="true"]')
    .evaluate((element) => window.getComputedStyle(element).gridTemplateColumns);
  expect(detailLayoutColumns.trim().split(/\s+/).length).toBe(1);

  const summaryMetricColumns = await page
    .locator('[data-node-team-summary-metrics="true"]')
    .evaluate((element) => window.getComputedStyle(element).gridTemplateColumns);
  expect(summaryMetricColumns.trim().split(/\s+/).length).toBe(1);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("agents workbench keeps mobile primary controls reachable", async ({
  page,
}) => {
  await mockTeamPageApis(page);
  const sentInputs: Array<{
    agent_id: string;
    input: string;
    session_id?: string;
  }> = [];

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });
  await page.route(/\/api\/agents\/[^/]+\/input(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const agentId = decodeURIComponent(
      url.pathname.match(/\/api\/agents\/([^/]+)\/input$/)?.[1] ?? ""
    );
    const payload = request.postDataJSON() as {
      input: string;
      session_id?: string;
    };
    sentInputs.push({
      agent_id: agentId,
      input: payload.input,
      session_id: payload.session_id,
    });
    await route.fulfill(jsonResponse({ status: "ok" }));
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/workspace", { waitUntil: "domcontentloaded" });

  const showAgentsToggle = page.getByRole("banner").getByRole("button", {
    name: "Show agents",
  });
  await expect(showAgentsToggle).toBeVisible();
  await expect(page.getByText("Coordinator Agent", { exact: true })).toBeVisible();

  const agentInput = page.getByPlaceholder(/Send input|Type a message \(tap Send/);
  await expect(agentInput).toBeVisible();
  await agentInput.fill("Summarize the current workspace state.");
  await page.getByRole("button", { name: "Send input", exact: true }).click();
  await expect
    .poll(() => sentInputs.length, { timeout: 10_000 })
    .toBe(1);
  expect(sentInputs[0]).toMatchObject({
    agent_id: "agent-coordinator-1",
    input: "Summarize the current workspace state.",
  });

  await showAgentsToggle.click();
  const hideAgentsToggle = page.getByRole("banner").getByRole("button", {
    name: "Hide agents",
  });
  await expect(hideAgentsToggle).toBeVisible();
  await expect(page.getByText("Worker Agent", { exact: true })).toBeVisible();
  await hideAgentsToggle.click();
  await expect(showAgentsToggle).toBeVisible();

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("team setup actions stay reachable through mobile shell-first creation and worker add", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamName = "Mobile Setup Team";

  await page.setViewportSize({ width: 390, height: 844 });
  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: teamName,
    goal: "Validate mobile setup actions after creating a shell-only team.",
  });

  const setupPanel = page.locator(".teams-main").filter({
    hasText: "No agents have joined this team yet.",
  });
  await expect(setupPanel.getByRole("button", { name: "Copy Existing Agent" })).toBeVisible();
  await expect(setupPanel.getByRole("button", { name: "Create New Agent" })).toBeVisible();

  await setupPanel.getByRole("button", { name: "Copy Existing Agent" }).click();
  const dialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Copy an existing agent into this Team." })
    .last();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Copy keeps the source agent unchanged.")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Move to Team (later)" })).toHaveCount(0);

  const dialogBox = await dialog.boundingBox();
  const viewportWidth = await page.evaluate(() => document.documentElement.clientWidth);
  expect(dialogBox).not.toBeNull();
  expect(dialogBox!.x).toBeGreaterThanOrEqual(0);
  expect(dialogBox!.x + dialogBox!.width).toBeLessThanOrEqual(viewportWidth + 1);

  await dialog.getByRole("button", { name: "Copy into Team" }).click();
  await expect(dialog).toBeHidden();

  const updates = fixture.getUpdateSpecPayloads();
  expect(updates).toHaveLength(1);
  expect(updates[0]?.payload.spec.coordinator_member_id).toBe("agent-forge-4");
  expect(updates[0]?.payload.spec.members[0]).toMatchObject({
    member_id: "agent-forge-4",
    role: "coordinator",
    description: "Copied from existing agent Coordinator Agent.",
  });

  await page.getByRole("button", { name: "Show teams panel" }).click();
  await selectedTeamMenuLocator(page).click();
  await page.getByRole("menuitem", { name: "Copy Existing Agent" }).click();
  const workerDialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Copy an existing agent into this Team." })
    .last();
  await expect(workerDialog).toBeVisible();
  await expect(workerDialog.getByText("new Team-owned worker agent").first()).toBeVisible();
  await workerDialog.getByLabel("Search existing agents").fill("agent-worker-1");
  await workerDialog.locator("li", { hasText: "agent-worker-1" }).getByRole("button").click();
  await workerDialog.getByRole("button", { name: "Copy into Team" }).click();
  await expect(workerDialog).toBeHidden();

  const finalUpdates = fixture.getUpdateSpecPayloads();
  expect(finalUpdates).toHaveLength(2);
  expect(finalUpdates[1]?.payload.spec.coordinator_member_id).toBe("agent-forge-4");
  expect(finalUpdates[1]?.payload.spec.members).toEqual([
    expect.objectContaining({
      member_id: "agent-forge-4",
      role: "coordinator",
    }),
    expect.objectContaining({
      member_id: "agent-forge-5",
      role: "worker",
      description: "Copied from existing agent Worker Agent.",
    }),
  ]);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});
