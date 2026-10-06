import { expect, test } from "./coverage";
import { mockNativeRecovery } from "./native_recovery_fixture";

test("recovery review uses the current owner and never replays input", async ({ page }) => {
  const state = await mockNativeRecovery(page);
  await page.goto(state.path);
  await expect(page.getByText("Interrupted work is retained.", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Review recovery", exact: true }).click();
  const confirm = page.getByRole("button", { name: "Confirm recovery review", exact: true });
  await expect(confirm).toBeDisabled();
  await page.getByLabel("Recovery note", { exact: true }).fill("Old process stopped; effects checked");
  await expect(confirm).toBeEnabled();
  await page.screenshot({ path: "/tmp/agenthub-native-recovery-after.png", fullPage: true });
  await confirm.click();
  await expect(page.getByText("Recovery review saved. Send a new instruction when ready.", { exact: true })).toBeVisible();
  expect(state.resolutions).toEqual([{
    local_session_id: state.localSessionId,
    target: state.target,
    note: "Old process stopped; effects checked",
  }]);
  expect(state.inputs).toHaveLength(0);
  expect(state.reads()).toBe(1);
});

test("recovery controls remain usable on a narrow workspace", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const state = await mockNativeRecovery(page);
  await page.goto(state.path);
  await page.getByRole("button", { name: "Review recovery", exact: true }).click();
  await page.getByLabel("Recovery note", { exact: true }).fill("Effects inspected");
  const confirm = page.getByRole("button", { name: "Confirm recovery review", exact: true });
  await expect(confirm).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: "/tmp/agenthub-native-recovery-mobile.png", fullPage: true });
  await confirm.click();
  await expect(page.getByText("Recovery review saved. Send a new instruction when ready.", { exact: true })).toBeVisible();
  expect(state.resolutions).toHaveLength(1);
  expect(state.inputs).toHaveLength(0);
});

test("viewers can read the thread without a recovery action", async ({ page }) => {
  const state = await mockNativeRecovery(page, "viewer");
  await page.goto(state.path);
  await expect(page.getByText("Interrupted work is retained.", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Review recovery", exact: true })).toHaveCount(0);
  expect(state.reads()).toBe(0);
  expect(state.resolutions).toHaveLength(0);
});
