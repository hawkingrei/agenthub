import { expect, test } from "./coverage";
import {
  gotoTeams,
  mockTeamPageApis,
  openTeamFromSelector,
  selectTeamChannelFromSidebar,
  teamChannelSidebarEntry,
} from "./team_page_helpers";
import {
  buildTeamChannelProfilePath,
  buildTeamChannelThreadPath,
} from "../../src/pages/team/team_route_helpers";

test("team channel sidebar helper does not select prefixed channel ids", async ({ page }) => {
  await page.setContent(`
    <aside class="teams-sidebar">
      <button type="button"># all-archive archived lane</button>
      <button type="button">Kanban Human task requests belong in # all.</button>
      <button type="button"># all default lane</button>
    </aside>
  `);

  await expect(teamChannelSidebarEntry(page, "all")).toHaveText("# all default lane");
  expect(() => teamChannelSidebarEntry(page, "# all")).toThrow(
    "Expected a raw channel id without the display prefix"
  );
});

test("team channels shows #all by default in sidebar", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-ch-default";
  const teamCreatedAt = fixture.now + 90;
  fixture.teams.push({
    id: teamId,
    name: "Default Channel Team",
    description: "channel default e2e",
    spec: {
      coordinator_member_id: "planner",
      members: [
        { member_id: "planner", role: "coordinator", model: "codex" },
        { member_id: "worker-1", role: "worker", model: "codex" },
      ],
      steps: [{ step_key: "coordinator_plan" }, { step_key: "worker_execute", member_id: "worker-1" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Default Channel Team");

  await expect(teamChannelSidebarEntry(page, "all")).toBeVisible();

  await selectTeamChannelFromSidebar(page, "all");
  await expect(page).toHaveURL(/workspace/);
});

test("team channels create, switch and delete custom channel", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-ch-crud";
  const teamCreatedAt = fixture.now + 90;
  fixture.teams.push({
    id: teamId,
    name: "Channel CRUD Team",
    description: "channel crud e2e",
    spec: {
      coordinator_member_id: "planner",
      members: [
        { member_id: "planner", role: "coordinator", model: "codex" },
        { member_id: "worker-1", role: "worker", model: "codex" },
      ],
      steps: [{ step_key: "coordinator_plan" }, { step_key: "worker_execute", member_id: "worker-1" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Channel CRUD Team");

  // Open the create channel form
  const createChannelButton = page.getByLabel("Create channel");
  await expect(createChannelButton).toBeVisible();
  await createChannelButton.click();

  // Fill in channel details
  const channelIdInput = page.getByLabel("Channel ID");
  await expect(channelIdInput).toBeVisible();
  await channelIdInput.fill("review");

  const channelDescInput = page.getByLabel("Channel Description");
  await expect(channelDescInput).toBeVisible();
  await channelDescInput.fill("Code review and PR discussion");

  // Submit
  await page.locator('button[type="submit"]').filter({ hasText: "Create channel" }).click();

  const reviewChannel = teamChannelSidebarEntry(page, "review");
  await expect(reviewChannel).toBeVisible();

  await selectTeamChannelFromSidebar(page, "review");
  await expect(page).toHaveURL(/\/channels\/review/);

  await selectTeamChannelFromSidebar(page, "all");

  // Delete the custom channel (hover to reveal delete button)
  await reviewChannel.hover();
  const deleteButton = page.getByLabel("Delete channel review");
  await expect(deleteButton).toBeVisible();

  // Mock the confirm dialog
  page.once("dialog", (dialog) => dialog.accept());
  await deleteButton.click();

  // The channel should be removed
  await expect(reviewChannel).toBeHidden();
});

test("team channels navigates to channel via url and opens thread", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-ch-thread";
  const teamCreatedAt = fixture.now + 90;
  fixture.teams.push({
    id: teamId,
    name: "Thread Team",
    description: "thread e2e",
    spec: {
      coordinator_member_id: "planner",
      members: [
        { member_id: "planner", role: "coordinator", model: "codex" },
        { member_id: "worker-1", role: "worker", model: "codex" },
      ],
      steps: [{ step_key: "coordinator_plan" }, { step_key: "worker_execute", member_id: "worker-1" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });
  fixture.seedTaskMessages(`task-${teamId}-1`, [
    {
      message_id: 5,
      conversation_id: `conversation-task-${teamId}-1`,
      task_id: `task-${teamId}-1`,
      from_actor_id: "planner",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "chat_message",
        text: "Root channel update",
      },
      created_at: fixture.now + 95,
    },
  ]);

  // Navigate directly to the canonical channel + thread URL.
  await page.goto(buildTeamChannelThreadPath(teamId, "all", 5));

  // The page should load with the channels lens active
  await expect(page).toHaveURL(/workspace/);
  await expect(page).toHaveURL(/\/channels\/all\/threads\/5/);
  await expect(page.getByText("Root channel update")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Reply in thread" })).toBeVisible();
});

test("team channels opens member profile from canonical channel url", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-ch-profile";
  const teamCreatedAt = fixture.now + 90;
  const taskId = `task-${teamId}-1`;
  const runId = `${teamId}-working-1`;
  fixture.teams.push({
    id: teamId,
    name: "Profile Route Team",
    description: "profile route e2e",
    spec: {
      coordinator_member_id: "agent-coordinator-1",
      members: [
        { member_id: "agent-coordinator-1", role: "coordinator", model: "codex" },
        {
          member_id: "agent-worker-1",
          role: "worker",
          model: "gemini",
          description: "Handles browser route validation",
        },
      ],
      steps: [
        { step_key: "coordinator_plan", member_id: "agent-coordinator-1" },
        { step_key: "worker_execute", member_id: "agent-worker-1" },
      ],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });
  fixture.seedRuns(teamId, [
    {
      id: runId,
      team_id: teamId,
      context_id: `ctx-${runId}`,
      status: "working",
      input: {},
      created_at: fixture.now + 100,
      started_at: fixture.now + 101,
      ended_at: null,
    },
  ]);

  const runsResponsePromise = page.waitForResponse(
    (response) =>
      response.url().includes(`/api/teams/${teamId}/runs`) &&
      response.request().method() === "GET"
  );
  const snapshotResponsePromise = page.waitForResponse(
    (response) =>
      response.url().includes(`/api/teams/runs/${runId}/snapshot`) &&
      response.request().method() === "GET"
  );
  await page.goto(buildTeamChannelProfilePath(teamId, "all", "agent-worker-1", taskId));
  const runsResponse = await runsResponsePromise;
  const runsPayload = (await runsResponse.json()) as unknown[];
  expect(runsPayload).toHaveLength(1);
  const snapshotResponse = await snapshotResponsePromise;
  const snapshotPayload = (await snapshotResponse.json()) as { members: unknown[] };
  expect(snapshotPayload.members).toHaveLength(2);

  await expect(page).toHaveURL(
    /\/channels\/all\/tasks\/task-team-ch-profile-1\/members\/agent-worker-1/
  );
  await expect(page.getByText("Agent Profile")).toBeVisible();
  await expect(page.getByText("Worker Agent agent-worker-1")).toBeVisible();
  await expect(page.getByText("Handles browser route validation")).toBeVisible();

  await page.getByRole("button", { name: "Close agent profile" }).click();
  await expect(page).toHaveURL(new RegExp(`/workspace/teams/${teamId}$`));
  await expect(page.getByText("Agent Profile")).toBeHidden();
});

test("non-default channel delete requires confirmation", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-ch-confirm";
  const teamCreatedAt = fixture.now + 90;
  fixture.teams.push({
    id: teamId,
    name: "Confirm Team",
    description: "confirm delete e2e",
    spec: {
      coordinator_member_id: "planner",
      members: [
        { member_id: "planner", role: "coordinator", model: "codex" },
      ],
      steps: [{ step_key: "coordinator_plan" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Confirm Team");

  // Create a custom channel
  const createChannelButton = page.getByLabel("Create channel");
  await createChannelButton.click();
  await page.getByLabel("Channel ID").fill("staging");
  await page.locator('button[type="submit"]').filter({ hasText: "Create channel" }).click();

  const stagingChannel = teamChannelSidebarEntry(page, "staging");
  await expect(stagingChannel).toBeVisible();

  // Cancel the delete dialog should keep the channel
  await stagingChannel.hover();
  const deleteButton = page.getByLabel("Delete channel staging");
  page.once("dialog", (dialog) => dialog.dismiss());
  await deleteButton.click();

  // Channel should still be visible
  await expect(stagingChannel).toBeVisible();
});
