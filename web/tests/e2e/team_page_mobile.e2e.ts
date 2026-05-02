import { expect, test } from "./coverage";
import {
  expectTeamRuntimeBadge,
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
  await expectTeamRuntimeBadge(page, "team running");

  const layoutColumns = await page.locator(".teams-layout").evaluate((element) => {
    return window.getComputedStyle(element).gridTemplateColumns;
  });
  expect(layoutColumns.trim().split(/\s+/).length).toBe(1);

  const titleBox = await page.getByRole("heading", { name: "Team Mobile", exact: true }).boundingBox();
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
  const fixture = await mockTeamPageApis(page);
  const remoteNodeId = "node-east";
  fixture.nodes.push({
    id: remoteNodeId,
    name: "Node East",
    grpc_target: "grpcs://node-east.internal:50051",
    tls_server_name: "node-east.internal",
    default_worktree_root: "/srv/agenthub/worktrees",
    last_seen_at: fixture.now - 60,
    is_main: false,
    created_at: fixture.now,
    updated_at: fixture.now,
  });
  const workerAgent = fixture.agents.find((agent) => agent.id === "agent-worker-1");
  if (workerAgent) {
    workerAgent.target_node_id = remoteNodeId;
  }
  fixture.teams.push({
    id: "team-node-mobile",
    name: "Node Mobile Team",
    description: "node mobile layout regression guard",
    spec: {
      coordinator_member_id: "agent-coordinator-1",
      members: [
        { member_id: "agent-coordinator-1", role: "coordinator", model: "codex" },
        { member_id: "agent-worker-1", role: "worker", model: "gemini" },
      ],
      steps: [{ step_key: "coordinator_plan" }],
    },
    created_at: fixture.now + 10,
    updated_at: fixture.now + 10,
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/workspace/nodes/${remoteNodeId}`, { waitUntil: "domcontentloaded" });

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
