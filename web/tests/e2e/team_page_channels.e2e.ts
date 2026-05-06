import { expect, test } from "./coverage";
import {
  gotoTeams,
  mockTeamPageApis,
  openTeamFromSelector,
  selectTeamChannelFromSidebar,
  teamChannelSidebarEntry,
} from "./team_page_helpers";

test("team channel sidebar helper does not select prefixed channel ids", async ({ page }) => {
  await page.setContent(`
    <aside class="teams-sidebar">
      <button type="button"># all-archive archived lane</button>
      <button type="button"># all default lane</button>
    </aside>
  `);

  await expect(teamChannelSidebarEntry(page, "all")).toHaveText("# all default lane");
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
  await expect(page).toHaveURL(/lens=channels/);
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
  await expect(page).toHaveURL(/channel=review/);

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

  // Navigate directly to a channel + thread URL
  await page.goto(`/workspace/teams/${teamId}?lens=channels&channel=all&thread=5`);

  // The page should load with the channels lens active
  await expect(page).toHaveURL(/lens=channels/);
  await expect(page).toHaveURL(/thread=5/);
  await expect(page.getByText("Root channel update")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Reply in thread" })).toBeVisible();
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
