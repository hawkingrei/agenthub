import { test, expect } from "./coverage";
import { jsonResponse, mockTeamPageApis } from "./team_page_fixture";

test("renders login shell", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
  });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
  await expect(page.getByPlaceholder("Username")).toBeVisible();
  await expect(page.getByPlaceholder("Password")).toBeVisible();
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("initializes root from the first-run setup shell", async ({ page }) => {
  const registerPayloads: unknown[] = [];
  await page.route("**/api/auth/status", async (route) => {
    await route.fulfill(jsonResponse({ root_initialized: false }));
  });
  await page.route("**/api/auth/register/start", async (route, request) => {
    registerPayloads.push(request.postDataJSON());
    await route.fulfill(jsonResponse({
      user_id: "root-user",
      token: "root-token",
      role: "root",
    }));
  });

  await page.goto("/");
  await expect(page.getByRole("heading", { name: "First-run setup" })).toBeVisible();
  await page.getByPlaceholder("Username").fill("root");
  await page.getByPlaceholder("Password").fill("password");
  await page.getByPlaceholder("Display Name").fill("Root User");
  await page.getByPlaceholder("Display Name").press("Enter");

  expect(registerPayloads).toEqual([
    {
      username: "root",
      display_name: "Root User",
      role: "root",
      password: "password",
    },
  ]);
});

test("renders authenticated workspace shell", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
  });
  await mockTeamPageApis(page);

  await page.goto("/workspace", { waitUntil: "domcontentloaded" });

  await expect(page.getByRole("button", { name: "Create Agent" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open workbench menu" })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("creates a Codex agent with an explicit runtime profile", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);

  await page.goto("/workspace", { waitUntil: "domcontentloaded" });
  await page.getByRole("button", { name: "Create Agent" }).click();

  const dialog = page.locator("[role='dialog']").filter({ hasText: "Runtime model" }).last();
  await expect(dialog.getByLabel("Runtime model")).toBeVisible();
  await expect(dialog.getByLabel("Thinking level")).toBeVisible();
  await dialog.getByPlaceholder("e.g. Alice").fill("Runtime Profile Agent");
  await dialog.getByLabel("Workspace path").fill("/workspace/runtime-profile-agent");
  await dialog.getByLabel("Runtime model").click();
  await page.getByRole("option", { name: "GPT-5.5", exact: true }).click();
  await dialog.getByLabel("Thinking level").click();
  await page.getByRole("option", { name: "High", exact: true }).click();
  await dialog.getByRole("button", { name: "Create Agent", exact: true }).click();

  await expect(dialog).toBeHidden();
  expect(fixture.agents.at(-1)).toMatchObject({
    name: "Runtime Profile Agent",
    runtime_model: "gpt-5.5",
    thinking_level: "high",
  });
});
