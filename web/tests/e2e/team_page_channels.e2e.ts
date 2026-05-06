import { expect, test } from "./coverage";
import {
  createTeamFromModal,
  gotoTeams,
  mockTeamPageApis,
  openTeamFromSelector,
} from "./team_page_helpers";

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
  await createTeamFromModal(page, { name: "Default Channel Team" });
  await openTeamFromSelector(page, "Default Channel Team");

  // The sidebar should show "# all" as the default channel
  const allChannel = page.locator("button").filter({ hasText: "# all" });
  await expect(allChannel.first()).toBeVisible();

  // Clicking "# all" should keep us in the channels lens
  await allChannel.first().click();
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
  await createTeamFromModal(page, { name: "Channel CRUD Team" });
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
  await page.getByRole("button", { name: "Create channel" }).click();

  // The new channel should appear in the sidebar
  const reviewChannel = page.locator("button").filter({ hasText: "# review" });
  await expect(reviewChannel.first()).toBeVisible();

  // Switch to the new channel
  await reviewChannel.first().click();
  await expect(page).toHaveURL(/channel=review/);

  // Switch back to # all
  const allChannel = page.locator("button").filter({ hasText: "# all" });
  await allChannel.first().click();

  // Delete the custom channel (hover to reveal delete button)
  await reviewChannel.first().hover();
  const deleteButton = page.getByLabel("Delete channel review");
  await expect(deleteButton).toBeVisible();

  // Mock the confirm dialog
  page.on("dialog", (dialog) => dialog.accept());
  await deleteButton.click();

  // The channel should be removed
  await expect(reviewChannel.first()).toBeHidden();
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

  // Navigate directly to a channel + thread URL
  await page.goto(`/workspace/teams/${teamId}?lens=channels&channel=all&thread=5`);

  // The page should load with the channels lens active
  await expect(page).toHaveURL(/lens=channels/);
  await expect(page).toHaveURL(/thread=5/);
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
  await createTeamFromModal(page, { name: "Confirm Team" });
  await openTeamFromSelector(page, "Confirm Team");

  // Create a custom channel
  const createChannelButton = page.getByLabel("Create channel");
  await createChannelButton.click();
  await page.getByLabel("Channel ID").fill("staging");
  await page.getByRole("button", { name: "Create channel" }).click();

  // Verify the channel appears
  const stagingChannel = page.locator("button").filter({ hasText: "# staging" });
  await expect(stagingChannel.first()).toBeVisible();

  // Cancel the delete dialog should keep the channel
  await stagingChannel.first().hover();
  const deleteButton = page.getByLabel("Delete channel staging");
  page.on("dialog", (dialog) => dialog.dismiss());
  await deleteButton.click();

  // Channel should still be visible
  await expect(stagingChannel.first()).toBeVisible();
});
