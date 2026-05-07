import { test, expect } from "./coverage";
import { mockTeamPageApis } from "./team_page_fixture";

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
