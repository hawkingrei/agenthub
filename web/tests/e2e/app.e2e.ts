import { test, expect } from "./coverage";

test("renders login shell", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "AgentHub" })).toBeVisible();
  await expect(page.getByPlaceholder("Username")).toBeVisible();
  await expect(page.getByPlaceholder("Password")).toBeVisible();
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
  expect(pageErrors).toEqual([]);
});
