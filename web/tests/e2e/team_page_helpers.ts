import { expect } from "./coverage";
import { UI_PREFS_STORAGE_KEY } from "../../src/ui/developer_mode";

export {
  type CreateTeamPayload,
  type TeamActorMessageRecord,
  type TeamRunRecord,
  buildTeamRun,
  jsonResponse,
  mockTeamPageApis,
} from "./team_page_fixture";

export function deriveAgentName(identity: string | undefined, workdir: string): string {
  const normalizedIdentity = identity?.trim();
  if (normalizedIdentity) {
    return normalizedIdentity;
  }
  const basename = workdir
    .split(/[\\/]+/)
    .filter(Boolean)
    .at(-1)
    ?.replace(/[-_]+/g, " ");
  return basename || "team member";
}

export function selectedTeamMenuLocator(page: import("@playwright/test").Page) {
  return page.getByRole("button", { name: /^Team menu:/ });
}

const TEAM_SELECTOR_ROUTE_PATTERN = /^\/(?:workspace\/)?teams\/?$/;
const TEAM_DETAIL_ROUTE_PATTERN = /^\/(?:workspace\/)?teams\/[^/]+$/;

function isTeamSelectorPath(pathname: string): boolean {
  return TEAM_SELECTOR_ROUTE_PATTERN.test(pathname);
}

function isTeamDetailPath(pathname: string): boolean {
  return TEAM_DETAIL_ROUTE_PATTERN.test(pathname);
}
export async function createTeamFromModal(
  page: import("@playwright/test").Page,
  options: {
    name: string;
    goal?: string;
  }
): Promise<void> {
  await openMainTeamAction(page, "New Team");
  const dialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Start with the mission" })
    .last();
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Team name").fill(options.name);
  if (options.goal) {
    await dialog.getByLabel("Team goal").fill(options.goal);
  }
  await dialog.getByRole("button", { name: "Create Team" }).click();
  await expect(dialog).toBeHidden();
  await expect(page).toHaveURL(/\/(?:workspace\/)?teams\/[^/?#]+(?:[?#].*)?$/);
  await expect.poll(() => isTeamDetailReady(page, options.name)).toBe(true);
  await expectAddAgentEntryVisible(page, options.name);
}

export type AddAgentEntryLane = "primary" | "menuItem" | "menuTrigger";

export async function restoreTeamAddAgentContext(
  page: import("@playwright/test").Page,
  teamName?: string
): Promise<void> {
  const showTeamsPanelButton = page.getByRole("button", {
    name: "Show teams panel",
    exact: true,
  });
  if (await showTeamsPanelButton.isVisible().catch(() => false)) {
    await showTeamsPanelButton.click();
  }

  const conversationSubject = page.getByRole("button", { name: "# all", exact: true });
  if (await conversationSubject.isVisible().catch(() => false)) {
    await conversationSubject.click();
    await page.waitForTimeout(150);
  }

  if (teamName) {
    await openTeamFromSelector(page, teamName);
  }
}

export async function waitForAddAgentEntryLane(
  page: import("@playwright/test").Page,
  teamName?: string
): Promise<AddAgentEntryLane> {
  const primaryButton = page
    .locator(".teams-main")
    .getByRole("button", { name: "Add Agent", exact: true })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", { name: "Add Agent", exact: true });
  const menuTrigger = page.getByRole("button", {
    name: /^Team menu:/,
  });

  const detectLane = async (): Promise<AddAgentEntryLane | "missing"> => {
    if (await visibleMenuItem.isVisible().catch(() => false)) {
      return "menuItem";
    }
    if (await primaryButton.isVisible().catch(() => false)) {
      return "primary";
    }
    if (await menuTrigger.isVisible().catch(() => false)) {
      return "menuTrigger";
    }
    return "missing";
  };

  const waitForLane = async (): Promise<void> => {
    await expect
      .poll(detectLane, {
        timeout: 5_000,
      })
      .not.toBe("missing");
  };

  try {
    await waitForLane();
  } catch {
    await restoreTeamAddAgentContext(page, teamName);
    await waitForLane();
  }

  const lane = await detectLane();
  if (lane !== "missing") {
    return lane;
  }
  throw new Error("Timed out waiting for an Add Agent entry point");
}

export async function createTeamMemberFromModal(
  page: import("@playwright/test").Page,
  options: {
    teamName?: string;
    workdir: string;
    model?: string;
    identity?: string;
  }
): Promise<void> {
  const openButtonLabel = "Add Agent";
  const confirmLabel = "Create Agent";
  const primaryOpenButton = page
    .locator(".teams-main")
    .getByRole("button", { name: openButtonLabel, exact: true })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", { name: openButtonLabel, exact: true });
  const menuTrigger = selectedTeamMenuLocator(page);
  const selectionError = page.getByText("Select a team first", { exact: true });
  const dialog = page
    .locator("[role='dialog']")
    .filter({ has: page.getByLabel("Agent name", { exact: true }) })
    .last();
  const waitForDialog = async (): Promise<boolean> => {
    try {
      await expect(dialog).toBeVisible({ timeout: 1_500 });
      return true;
    } catch {
      return false;
    }
  };
  const tryOpen = async (
    action: () => Promise<void>,
    options?: { waitForTeamSelection?: boolean }
  ): Promise<boolean> => {
    await action();
    if (await waitForDialog()) {
      return true;
    }
    if (options?.waitForTeamSelection && (await selectionError.isVisible().catch(() => false))) {
      await expect(page.getByRole("heading", { name: /.+/, exact: false }).first()).toBeVisible();
    }
    return false;
  };
  const openFromVisibleLane = async (): Promise<boolean> => {
    const lane = await waitForAddAgentEntryLane(page, options.teamName);
    if (lane === "menuItem") {
      return tryOpen(async () => visibleMenuItem.click());
    }
    if (lane === "primary") {
      return tryOpen(async () => primaryOpenButton.click(), {
        waitForTeamSelection: true,
      });
    }

    await expect(menuTrigger).toBeVisible();
    await openSelectedTeamMenu(page);
    const menuItem = page.getByRole("menuitem", { name: openButtonLabel, exact: true });
    await expect(menuItem).toBeVisible();
    return tryOpen(async () => menuItem.click());
  };

  if (!(await openFromVisibleLane())) {
    await page.waitForTimeout(150);
    if (!(await openFromVisibleLane())) {
      await openSelectedTeamMenu(page);
      await expect(visibleMenuItem).toBeVisible();
      await visibleMenuItem.click();
    }
  }
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Agent name").fill(deriveAgentName(options.identity, options.workdir));
  if (options.identity) {
    await dialog.getByLabel("Identity").fill(options.identity);
  }
  if (options.model && options.model !== "codex") {
    await dialog.getByLabel("Role model").selectOption(options.model);
  }
  await dialog.getByLabel(/Workdir/).fill(options.workdir);
  await dialog.getByRole("button", { name: confirmLabel }).click();
  await expect(dialog).toBeHidden();
}

export async function openTeamFromSelector(
  page: import("@playwright/test").Page,
  teamName: string
): Promise<void> {
  const pathname = new URL(page.url()).pathname;
  if (!isTeamSelectorPath(pathname)) {
    const selectorButton = page.getByRole("button", {
      name: "Show teams panel",
      exact: true,
    });
    if ((await selectorButton.count()) > 0) {
      await selectorButton.first().click();
    } else {
      await page.goto("/workspace/teams", { waitUntil: "domcontentloaded" });
    }
  }
  await expect(page).toHaveURL(/\/(?:workspace\/)?teams(?:[/?#]|$)/);
  const selectorPanel = teamSelectorPanel(page);
  await expect(selectorPanel).toBeVisible();
  const filterInput = selectorPanel.getByLabel(/Filter teams|Search teams/);
  if ((await filterInput.count()) > 0) {
    await filterInput.fill(teamName);
  }
  const selectorTeamButton = selectorPanel
    .locator('[data-team-selector-entry="true"]', { hasText: teamName })
    .first();
  await expect(selectorTeamButton).toBeVisible();
  const selectorTeamId = await selectorTeamButton.getAttribute("data-team-id");
  expect(selectorTeamId).toBeTruthy();
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await selectorTeamButton.click({ timeout: 1_500, force: attempt > 0 });
    } catch {
      await page.goto(`/workspace/teams/${selectorTeamId}`, { waitUntil: "domcontentloaded" });
    }
    await page.waitForTimeout(150);
    if (await isTeamDetailReady(page, teamName)) {
      return;
    }
  }
  await page.goto(`/workspace/teams/${selectorTeamId}`, { waitUntil: "domcontentloaded" });
  await expect.poll(() => isTeamDetailReady(page, teamName)).toBe(true);
}

export async function isTeamDetailReady(
  page: import("@playwright/test").Page,
  expectedTeamName?: string
): Promise<boolean> {
  try {
    const detailPath = new URL(page.url()).pathname;
    if (!isTeamDetailPath(detailPath)) {
      return false;
    }
    const teamsMain = page.locator(".teams-main").first();
    if ((await teamsMain.count()) === 0) {
      return false;
    }
    const teamsMainVisible = await teamsMain.isVisible().catch(() => false);
    if (!teamsMainVisible) {
      return false;
    }
    const pageText = await page.locator("body").textContent();
    if (!pageText) {
      return false;
    }
    void expectedTeamName;
    return (
      !pageText.includes("Loading team workspace") &&
      !pageText.includes("This team is unavailable")
    );
  } catch {
    return false;
  }
}

export async function gotoTeams(page: import("@playwright/test").Page): Promise<void> {
  await page.goto("/teams", { waitUntil: "domcontentloaded" });
  await expect(teamSelectorPanel(page)).toBeVisible();
}

export function teamSelectorPanel(page: import("@playwright/test").Page) {
  return page.locator("section").filter({
    has: page.getByRole("button", { name: "New Team", exact: true }),
  }).first();
}

export async function selectAgentFromSidebar(
  page: import("@playwright/test").Page,
  agentLabel: string
): Promise<void> {
  const sidebar = page.locator(".teams-sidebar");
  const subjectsScopeButton = sidebar
    .getByLabel("Team sidebar scope switch")
    .getByText("Channels & Agents", { exact: true })
    .first();
  if (await subjectsScopeButton.isVisible()) {
    await subjectsScopeButton.click();
  }

  const agentsToggle = sidebar.getByRole("button", { name: "Toggle agents section" });
  if ((await agentsToggle.getAttribute("aria-expanded")) !== "true") {
    await agentsToggle.click();
  }

  const agentItem = sidebar
    .locator("button", { hasText: agentLabel })
    .first();
  await expect(agentItem).toBeVisible();
  await agentItem.click();
  const teamsMain = page.locator(".teams-main");
  await expect(teamsMain.locator(".acp-conversation").first()).toBeVisible();
}

export async function expectTeamRuntimeBadge(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  const statusText = page.getByText(label, { exact: false }).first();
  if (await statusText.isVisible().catch(() => false)) {
    await expect(statusText).toBeVisible();
    return;
  }

  const expectedMenuItem =
    label === "team running"
      ? page.getByRole("menuitem", { name: "Stop Team", exact: true })
      : label === "team stopped"
        ? page.getByRole("menuitem", { name: "Start Team", exact: true })
        : null;
  if (!expectedMenuItem) {
    await expect(statusText).toBeVisible();
    return;
  }

  await openSelectedTeamMenu(page);
  await expect(expectedMenuItem).toBeVisible();
}

export async function openSelectedTeamMenu(
  page: import("@playwright/test").Page
): Promise<void> {
  const trigger = selectedTeamMenuLocator(page);
  await expect(trigger).toBeVisible();
  if ((await trigger.getAttribute("aria-expanded")) !== "true") {
    await trigger.click();
  }
}

export async function clickSelectedTeamMenuItem(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  await openSelectedTeamMenu(page);
  const item = page.getByRole("menuitem", { name: label, exact: true });
  await expect(item).toBeVisible();
  await item.click();
}

export async function expectAddAgentEntryVisible(
  page: import("@playwright/test").Page,
  teamName?: string
): Promise<void> {
  const lane = await waitForAddAgentEntryLane(page, teamName);
  if (lane === "primary" || lane === "menuItem") {
    return;
  }
  const menuTrigger = selectedTeamMenuLocator(page);
  await expect(menuTrigger).toBeVisible();
  await openSelectedTeamMenu(page);
  await expect(page.getByRole("menuitem", { name: "Add Agent", exact: true })).toBeVisible();
}

export async function openKanbanDeveloperTools(
  page: import("@playwright/test").Page
): Promise<void> {
  const compilePreviewButton = page.getByRole("button", { name: "Compile Preview", exact: true });
  const compilePreviewVisible = await compilePreviewButton.isVisible().catch(() => false);
  if (!compilePreviewVisible) {
    const firstTaskCard = page.locator('[data-team-surface="kanban"] .team-item').first();
    await expect(firstTaskCard).toBeVisible();
    await firstTaskCard.click();

    const taskDetailDialog = page.getByRole("dialog", { name: "Task detail" });
    await expect(taskDetailDialog).toBeVisible();

    const developerToolsSummary = taskDetailDialog.locator("summary").filter({
      has: page.getByText("Developer tools", { exact: true }),
    });
    await expect(developerToolsSummary).toBeVisible();
    await developerToolsSummary.click();
  }
  await expect(compilePreviewButton).toBeVisible();
  await expect(compilePreviewButton).toBeEnabled();
}

export async function selectPrimaryTeamEntryFromSidebar(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  const sidebar = page.locator(".teams-sidebar");
  const entry = sidebar
    .locator("button", { hasText: label })
    .first();
  await expect(entry).toBeVisible();
  await entry.click();
}

export async function openMainTeamAction(
  page: import("@playwright/test").Page,
  label: string,
  allowSidebarReset = true
): Promise<void> {
  const taskDetailDialog = page.getByRole("dialog", { name: "Task detail" });
  if (await taskDetailDialog.isVisible().catch(() => false)) {
    const closeButton = taskDetailDialog.locator(".mantine-Modal-close").first();
    if (await closeButton.isVisible().catch(() => false)) {
      await closeButton.click();
      await expect(taskDetailDialog).toBeHidden();
    } else {
      await page.keyboard.press("Escape").catch(() => {});
      await expect(taskDetailDialog).toBeHidden();
    }
  }

  const teamsMain = page.locator(".teams-main");
  const scope = (await teamsMain.count()) > 0 ? teamsMain : page.locator("body");
  const tab = scope.getByRole("tab", { name: label, exact: true }).first();
  if ((await tab.count()) > 0) {
    await expect(tab).toBeVisible();
    await tab.click();
    return;
  }

  const button = scope.getByRole("button", { name: label, exact: true }).first();
  if ((await button.count()) > 0) {
    await expect(button).toBeVisible();
    await button.click();
    return;
  }

  const workflowTabButton = page
    .locator('[data-team-surface="workflow-tabs"]')
    .getByRole("button", { name: label, exact: true })
    .first();
  if ((await workflowTabButton.count()) > 0) {
    await expect(workflowTabButton).toBeVisible();
    await workflowTabButton.click();
    return;
  }

  const moreTrigger = page
    .getByRole("button", { name: /^(More|Open more workspace actions)$/ })
    .first();
  if ((await moreTrigger.count()) > 0) {
    await expect(moreTrigger).toBeVisible();
    await moreTrigger.click();
    const menuItem = page.getByRole("menuitem", { name: label, exact: true }).first();
    const menuItemVisible =
      (await menuItem.count()) > 0 &&
      (await menuItem.isVisible().catch(() => false));
    if (menuItemVisible) {
      await expect(menuItem).toBeVisible();
      await menuItem.click();
      return;
    }
    await page.waitForTimeout(150);
    const menuItemVisibleAfterTick =
      (await menuItem.count()) > 0 &&
      (await menuItem.isVisible().catch(() => false));
    if (menuItemVisibleAfterTick) {
      await expect(menuItem).toBeVisible();
      await menuItem.click();
      return;
    }
    await page.keyboard.press("Escape").catch(() => {});
  }

  if (allowSidebarReset) {
    const sidebarConversationEntry = page
      .locator(".teams-sidebar")
      .locator("button", { hasText: "# all" })
      .first();
    if ((await sidebarConversationEntry.count()) > 0) {
      await expect(sidebarConversationEntry).toBeVisible();
      await sidebarConversationEntry.click();
      await openMainTeamAction(page, label, false);
      return;
    }
  }

  throw new Error(`Team action not found: ${label}`);
}

export async function openAdvancedView(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  const trigger = page
    .getByRole("button", { name: /^(More|Open more workspace actions)$/ })
    .first();
  await expect(trigger).toBeVisible();
  await trigger.click();
  const menuItem = page.getByRole("menuitem", { name: label, exact: true });
  await expect(menuItem).toBeVisible();
  await menuItem.click();
}

export async function enableDeveloperMode(
  page: import("@playwright/test").Page
): Promise<void> {
  await page.addInitScript((storageKey: string) => {
    window.localStorage.setItem(
      storageKey,
      JSON.stringify({ developerMode: true })
    );
  }, UI_PREFS_STORAGE_KEY);
}
