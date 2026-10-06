import fs from "node:fs";
import path from "node:path";
import { expect, test } from "./coverage";
import { buildWorkspacePath } from "../../src/app_route_selection";

test("actual standalone recovery waits for explicit input and survives refresh", async ({ page }) => {
  const directory = process.env.STANDALONE_NATIVE_BROWSER_DIR;
  test.skip(!directory, "requires the opt-in standalone continuity Rust fixture");
  test.setTimeout(60_000);
  const root = directory!;
  const fixture = JSON.parse(fs.readFileSync(path.join(root, "ready.json"), "utf8"));
  await page.addInitScript(auth => {
    window.localStorage.setItem("agenthub_auth", JSON.stringify(auth));
  }, fixture.auth);
  const resolutions: unknown[] = [];
  const inputs: Array<{ input: string; session_id: string }> = [];
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (request.method() !== "POST") return;
    if (request.url().endsWith("/runtime/recovery")) resolutions.push(request.postDataJSON());
    if (request.url().endsWith("/input")) inputs.push(request.postDataJSON());
  });
  await page.goto(fixture.origin + buildWorkspacePath(fixture.agent_id));
  await page.getByRole("button", { name: "Review recovery", exact: true }).click();
  await expect(page.getByText("1 earlier approval has an unconfirmed outcome.", { exact: true })).toBeVisible();
  await page.getByLabel("Recovery note", { exact: true }).fill("Old executor retired; one append inspected; do not repeat it");
  await page.screenshot({ path: path.join(root, "before-review.png"), fullPage: true });
  await page.getByRole("button", { name: "Confirm recovery review", exact: true }).click();
  await expect(page.getByText("Recovery review saved. Send a new instruction when ready.", { exact: true })).toBeVisible();
  expect(resolutions).toEqual([{
    local_session_id: fixture.local_session_id,
    target: fixture.target,
    note: "Old executor retired; one append inspected; do not repeat it",
  }]);
  expect(inputs).toHaveLength(0);
  fs.writeFileSync(path.join(root, "reviewed"), "");
  await expect.poll(() => fs.existsSync(path.join(root, "review-verified"))).toBe(true);
  await page.getByPlaceholder("Send input (Enter to send, Shift+Enter for newline)", { exact: true }).fill("explicit-after-recovery");
  await page.getByRole("button", { name: "Send input", exact: true }).click();
  await expect(page.getByText("Standalone continuity marker", { exact: true })).toBeVisible();
  expect(inputs).toHaveLength(1);
  expect(inputs[0]).toMatchObject({ input: "explicit-after-recovery", session_id: fixture.local_session_id });
  fs.writeFileSync(path.join(root, "input-sent"), "");
  await expect.poll(() => fs.existsSync(path.join(root, "completed")), { timeout: 15_000 }).toBe(true);
  await page.reload();
  await expect(page.getByText("Standalone continuity marker", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Review recovery", exact: true }).click();
  await expect(page.getByText("No interrupted work needs reconciliation.", { exact: true })).toBeVisible();
  await page.screenshot({ path: path.join(root, "after-review.png"), fullPage: true });
  expect(inputs).toHaveLength(1);
  expect(resolutions).toHaveLength(1);
  expect(errors).toEqual([]);
  fs.writeFileSync(path.join(root, "stop"), "");
});
