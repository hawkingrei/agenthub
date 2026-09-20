import { expect, test } from "./coverage";
import { mockLoopWorkspace } from "./team_loop_fixture";
import { buildTeamMemberWorkspacePath } from "../../src/pages/team/team_route_helpers";

test("offline loop settings, work, and retained history remain separate", async ({
  page,
}) => {
  const state = await mockLoopWorkspace(page);
  const path = buildTeamMemberWorkspacePath(
    state.teamId,
    state.actorId,
    "overview",
  );
  await page.goto(path);
  await expect(
    page.getByText("Process: stopped", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Execution: disabled", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Completion proposed; task review remains separate"),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Review retained evidence" }),
  ).toBeVisible();

  await page
    .getByRole("button", { name: "Execution settings", exact: true })
    .click();
  await page.getByLabel("Activations per window").fill("");
  await expect(
    page.getByRole("button", { name: "Save execution settings" }),
  ).toBeDisabled();
  await page.getByLabel("Activations per window").fill("5");
  await page.getByLabel("Window in seconds").fill("120");
  await page
    .getByRole("combobox", { name: "Session policy", exact: true })
    .click();
  await page.getByRole("option", { name: "Resume when supported" }).click();
  await page.getByRole("button", { name: "Save execution settings" }).click();
  await expect.poll(() => state.updates.length).toBe(1);
  expect(state.updates[0]).toMatchObject({
    expected_revision: 1,
    session_policy: "resume",
    limits: {
      activations_per_actor: 5,
      window_seconds: 120,
      activations_per_team: 120,
    },
  });
  await page.getByRole("button", { name: "Close settings" }).click();
  await page.getByRole("button", { name: "Enable execution" }).click();
  await expect(
    page.getByText("Execution: enabled", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Suspend execution" }).click();
  await expect(
    page.getByText("Execution: suspended", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Activate member", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText(
    "Activation request accepted",
  );
  await expect(
    page.getByText("Execution: suspended", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Resume execution" }).click();
  await expect(
    page.getByText("Execution: enabled", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Process: stopped", { exact: true }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Load more wake records" }).click();
  await expect(page.getByText("Task condition: in review")).toBeVisible();
  await expect(page.getByText("Waiting for a new thread reply")).toBeVisible();
  await expect(
    page.getByText("Waiting for App event: build.finished (app-release)"),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Inspect activation" })
    .first()
    .click();
  await expect(page.getByText("app_write: outcome unknown")).toBeVisible();
  await expect(
    page.getByText(
      "App app-release · build.finished · Event release-7 · Cursor 7 · Version 2",
    ),
  ).toBeVisible();
  await page.getByRole("button", { name: "More trigger sources" }).click();
  await page.getByRole("button", { name: "More lifecycle events" }).click();
  await page.getByRole("button", { name: "More tool observations" }).click();
  await expect(
    page.getByText("read_result: succeeded · 1000 ms"),
  ).toBeVisible();
  // Development StrictMode may restart the initial read; explicit page cursors must stay exact.
  expect(state.pages.source.filter((cursor) => cursor !== null)).toEqual([
    "source-1",
  ]);
  expect(state.pages.event.filter((cursor) => cursor !== null)).toEqual(["0"]);
  expect(state.pages.tool.filter((cursor) => cursor !== null)).toEqual(["0"]);
  await page.getByRole("button", { name: "Close details" }).click();
  await page.getByRole("button", { name: "Load older activations" }).click();
  await expect(page.getByText("Waiting for external event")).toBeVisible();
  await expect(
    page.getByText(/Automatic refresh pauses while viewing older records/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Refresh history" }).click();
  await expect(page.getByText("Waiting for external event")).toBeHidden();

  await page.getByRole("button", { name: "Edit profile", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByLabel("Description", { exact: true })
    .fill("Updated while stopped");
  await expect(dialog.getByLabel("Prompt", { exact: true })).toHaveValue("");
  await dialog.getByRole("button", { name: /Save/ }).click();
  await expect(dialog).toBeHidden();
  await expect(
    page.getByText("Updated while stopped", { exact: true }),
  ).toBeVisible();
  const spec = state.fixture.getUpdateSpecPayloads().at(-1)?.payload.spec;
  expect(spec?.execution_mode).toBe("loop");
  expect(
    spec?.members.find((member) => member.member_id === state.actorId),
  ).not.toHaveProperty("prompt");
  await page
    .getByRole("button", { name: "Review retained evidence", exact: true })
    .click();
  await expect(page).toHaveURL(/\/tasks\/retained-task$/);
});

test("uncertain activation retries the same request after a page reload", async ({
  page,
}) => {
  const state = await mockLoopWorkspace(page);
  state.configuration.policy!.state = "suspended";
  state.loseNextActivationResponse();
  await page.goto(
    buildTeamMemberWorkspacePath(state.teamId, state.actorId, "overview"),
  );
  await page
    .getByRole("button", { name: "Activate member", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Retry activation request" }),
  ).toBeVisible();
  expect(state.activationKeys).toHaveLength(1);
  await page.reload();
  await page.getByRole("button", { name: "Retry activation request" }).click();
  await expect(page.getByRole("status")).toContainText("existing request");
  expect(state.activationKeys).toEqual([
    state.activationKeys[0],
    state.activationKeys[0],
  ]);
  await expect(
    page.getByText("Execution: suspended", { exact: true }),
  ).toBeVisible();
  expect(state.updates).toHaveLength(0);
});
