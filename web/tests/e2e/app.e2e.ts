import { test, expect } from "@playwright/test";

test("renders login shell", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "AgentHub" })).toBeVisible();
  await expect(page.getByPlaceholder("Username")).toBeVisible();
  await expect(page.getByPlaceholder("Password")).toBeVisible();
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
});
