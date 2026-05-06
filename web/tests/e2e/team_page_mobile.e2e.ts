import { expect, test } from "./coverage";
import {
  createTeamFromModal,
  gotoTeams,
  mockTeamPageApis,
  openMainTeamAction,
  openTeamFromSelector,
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
  const lensesBox = await page.locator('[data-workspace-shell-lenses="true"]').boundingBox();
  expect(titleBox).not.toBeNull();
  expect(actionsBox).not.toBeNull();
  expect(lensesBox).not.toBeNull();
  expect(actionsBox!.y).toBeGreaterThan(titleBox!.y + 2);
  expect(lensesBox!.y).toBeGreaterThan(actionsBox!.y + 2);

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

test("node detail keeps mobile detail surfaces stacked without horizontal overflow", async ({
  page,
}) => {
  await mockTeamPageApis(page);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/workspace/nodes", { waitUntil: "domcontentloaded" });

  await expect(page.getByText("Node Detail", { exact: true })).toBeVisible();
  await expect(page.getByText("Detected Runtimes", { exact: true })).toBeVisible();
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

test("team setup actions stay reachable after mobile shell-first creation", async ({
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
  await expect(setupPanel.getByRole("button", { name: "Add First Coordinator Agent" })).toBeVisible();

  await setupPanel.getByRole("button", { name: "Copy Existing Agent" }).click();
  const dialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Copy an existing agent into this team." })
    .last();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Copy keeps the source agent unchanged.")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Move to Team (later)" })).toBeDisabled();

  const dialogBox = await dialog.boundingBox();
  expect(dialogBox).not.toBeNull();
  expect(dialogBox!.x).toBeGreaterThanOrEqual(0);
  expect(dialogBox!.x + dialogBox!.width).toBeLessThanOrEqual(390);

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

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});
