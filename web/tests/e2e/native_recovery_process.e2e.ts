import fs from "node:fs";
import path from "node:path";
import { expect, test } from "./coverage";
import { buildTeamMemberWorkspacePath } from "../../src/pages/team/team_route_helpers";

test("actual restarted runtime reconciles through the browser without replay", async ({ page }) => {
  const directory = process.env.LOOP_NATIVE_BROWSER_DIR;
  test.skip(!directory, "requires the opt-in native continuity Rust fixture");
  const root = directory!;
  const fixture = JSON.parse(fs.readFileSync(path.join(root, "ready.json"), "utf8"));
  await page.addInitScript(auth => {
    window.localStorage.setItem("agenthub_auth", JSON.stringify(auth));
  }, fixture.auth);
  const resolutions: unknown[] = [];
  const inputs: string[] = [];
  page.on("request", request => {
    if (request.method() !== "POST") return;
    if (request.url().endsWith("/runtime/recovery")) resolutions.push(request.postDataJSON());
    if (request.url().endsWith("/input")) inputs.push(request.url());
  });
  await page.goto(fixture.origin + buildTeamMemberWorkspacePath(fixture.team_id, "worker", "agent_acp"));
  await page.getByRole("button", { name: "Review recovery", exact: true }).click();
  await expect(page.getByText("1 earlier approval has an unconfirmed outcome.", { exact: true })).toBeVisible();
  await page.getByLabel("Recovery note", { exact: true }).fill("Old executor retired; one recorded append inspected; do not repeat it");
  await page.screenshot({ path: path.join(root, "before-review.png"), fullPage: true });
  const response = page.waitForResponse(response => response.request().method() === "POST" && response.url().endsWith("/runtime/recovery"));
  await page.getByRole("button", { name: "Confirm recovery review", exact: true }).click();
  expect((await response).status()).toBe(200);
  expect(resolutions).toEqual([{
    local_session_id: fixture.local_session_id,
    target: fixture.target,
    note: "Old executor retired; one recorded append inspected; do not repeat it",
  }]);
  expect(inputs).toHaveLength(0);
  fs.writeFileSync(path.join(root, "reviewed"), "");
  await expect.poll(() => fs.existsSync(path.join(root, "completed")), { timeout: 15_000 }).toBe(true);
  const eventsResponse = await page.request.get(`${fixture.origin}/api/agents/worker/events?limit=100`, {
    headers: { Authorization: `Bearer ${fixture.auth.token}` },
  });
  expect(eventsResponse.ok()).toBe(true);
  const events: Array<{ session_id: string; message: string }> = await eventsResponse.json();
  const retained = events.find(event => JSON.parse(event.message).text === "Persistent native continuity marker");
  expect(retained).toBeDefined();
  expect(retained!.session_id).not.toBe(fixture.local_session_id);
  expect(JSON.parse(retained!.message).meta.provider_runtime.native_session_id).toBe(fixture.target.session_id);
  await page.reload();
  // Process diagnostics select a live owner. Exited loop outcomes have their own durable view.
  await page.getByRole("button", { name: "Member configuration and history", exact: true }).click();
  await expect(page.getByText("Activation history", { exact: true })).toBeVisible();
  await expect(page.getByText("Waiting for input", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Inspect activation", exact: true })).toHaveCount(3);
  await page.getByText("Waiting for input", { exact: true }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(root, "after-review.png"), fullPage: true });
  expect(inputs).toHaveLength(0);
  fs.writeFileSync(path.join(root, "stop"), "");
});
