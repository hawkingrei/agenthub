import { expect } from "./coverage";
import type { Locator, Page } from "@playwright/test";
import {
  buildTeamChannelPath,
  buildTeamDetailPath,
  buildTeamLensNavigationPath,
  buildTeamSearchCompatibilityPath,
  buildTeamSelectorPath,
  buildTeamTabCompatibilityPath,
  resolveTeamRoute,
  type WorkspaceLens,
} from "../../src/pages/team/team_route_helpers";
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
  return page.getByRole("button", { name: /^Open controls for / });
}

const TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN =
  /^(Create New Agent|Add First Coordinator Agent|Add Worker Agent|Add Agent)$/;
const TEAM_CREATE_AGENT_CONFIRM_LABEL_PATTERN =
  /^(Create Agent|Create Coordinator Agent|Create Worker Agent)$/;
const TEAM_MEMBER_NAME_LABEL_PATTERN = /^(Name|Agent name)$/;
const TEAM_MEMBER_DESCRIPTION_LABEL_PATTERN = /^(Description|Identity)$/;
const TEAM_MEMBER_RUNTIME_LABEL_PATTERN = /^(Runtime|Role model)$/;
const TEAM_MEMBER_WORKSPACE_LABEL_PATTERN =
  /^(Workspace path|Workdir(?: \(optional override\))?)$/;

const TEAM_SELECTOR_ROUTE_PATTERN = /^\/(?:workspace\/)?teams\/?$/;
const TEAM_DETAIL_ROUTE_PATTERN = /^\/(?:workspace\/)?teams\/[^/]+$/;
const TEAM_DETAIL_READY_TIMEOUT_MS = 15_000;

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
  if (await teamForgeDialog(page).isVisible().catch(() => false)) {
    await expect(teamForgeDialog(page)).toBeVisible();
    return;
  }
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

  const conversationSubject = teamChannelSidebarEntry(page, "all");
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
    .getByRole("button", { name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", {
    name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN,
  });
  const menuTrigger = selectedTeamMenuLocator(page);

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
  throw new Error("Timed out waiting for a Team add-agent entry point");
}

export async function createTeamMemberFromModal(
  page: import("@playwright/test").Page,
  options: {
    teamName?: string;
    workdir: string;
    model?: string;
    runtimeModel?: string;
    thinkingLevel?: "low" | "medium" | "high" | "max";
    identity?: string;
  }
): Promise<void> {
  const primaryOpenButton = page
    .locator(".teams-main")
    .getByRole("button", { name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", {
    name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN,
  });
  const menuTrigger = selectedTeamMenuLocator(page);
  const selectionError = page.getByText("Select a team first", { exact: true });
  const dialog = teamForgeDialog(page);
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
    const menuItem = page.getByRole("menuitem", {
      name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN,
    });
    await expect(menuItem).toBeVisible();
    return tryOpen(async () => menuItem.click());
  };

  if (!(await waitForDialog()) && !(await openFromVisibleLane())) {
    await page.waitForTimeout(150);
    if (!(await waitForDialog()) && !(await openFromVisibleLane())) {
      await openSelectedTeamMenu(page);
      await expect(visibleMenuItem).toBeVisible();
      await visibleMenuItem.click();
    }
  }
  await expect(dialog).toBeVisible();
  await dialog
    .getByLabel(TEAM_MEMBER_NAME_LABEL_PATTERN)
    .fill(deriveAgentName(options.identity, options.workdir));
  if (options.identity) {
    await dialog.getByLabel(TEAM_MEMBER_DESCRIPTION_LABEL_PATTERN).fill(options.identity);
  }
  if (options.model && options.model !== "codex") {
    await dialog.getByLabel(TEAM_MEMBER_RUNTIME_LABEL_PATTERN).selectOption(options.model);
  }
  if (options.runtimeModel) {
    const runtimeModel = dialog.getByLabel("Runtime model");
    if (options.model === "codex") {
      await runtimeModel.click();
      await runtimeModel.fill(options.runtimeModel);
      await page
        .getByRole("option", { name: new RegExp(options.runtimeModel, "i") })
        .click();
    } else {
      await runtimeModel.fill(options.runtimeModel);
    }
  }
  if (options.thinkingLevel) {
    await dialog.getByLabel("Thinking level").click();
    const thinkingLevelLabel =
      options.thinkingLevel.slice(0, 1).toUpperCase() + options.thinkingLevel.slice(1);
    await page.getByRole("option", { name: thinkingLevelLabel, exact: true }).click();
  }
  await dialog.getByLabel(TEAM_MEMBER_WORKSPACE_LABEL_PATTERN).fill(options.workdir);
  await dialog
    .getByRole("button", { name: TEAM_CREATE_AGENT_CONFIRM_LABEL_PATTERN })
    .click();
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
      await page.goto(buildTeamSelectorPath(), { waitUntil: "domcontentloaded" });
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
  const selectedTeamId = selectorTeamId ?? "";
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await selectorTeamButton.click({ timeout: 1_500, force: attempt > 0 });
    } catch {
      await page.goto(buildTeamDetailPath(selectedTeamId), { waitUntil: "domcontentloaded" });
    }
    await page.waitForTimeout(150);
    if (await isTeamDetailReady(page, teamName)) {
      return;
    }
  }
  await page.goto(buildTeamDetailPath(selectedTeamId), { waitUntil: "domcontentloaded" });
  await expect
    .poll(() => isTeamDetailReady(page, teamName), {
      timeout: TEAM_DETAIL_READY_TIMEOUT_MS,
    })
    .toBe(true);
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
  await page.goto(buildTeamSelectorPath(), { waitUntil: "domcontentloaded" });
  await expect(teamSelectorPanel(page)).toBeVisible();
}

export function teamSelectorPanel(page: import("@playwright/test").Page) {
  return page.locator("section").filter({
    has: page.getByRole("button", { name: "New Team", exact: true }),
  }).first();
}

function escapeCssAttributeValue(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

export async function selectAgentFromSidebar(
  page: import("@playwright/test").Page,
  agentLabel: string
): Promise<void> {
  const sidebar = page.locator(".teams-sidebar");
  await revealTeamSidebarSubject(page, "agents");

  const agentItemByMemberId = sidebar
    .locator(`[data-team-member-id="${escapeCssAttributeValue(agentLabel)}"]`)
    .first();
  const agentItemByLabel = sidebar.locator("button", { hasText: agentLabel }).first();
  await expect
    .poll(async () => {
      if (await agentItemByMemberId.isVisible().catch(() => false)) {
        return "member-id";
      }
      if (await agentItemByLabel.isVisible().catch(() => false)) {
        return "label";
      }
      return "missing";
    }, { timeout: 5000 })
    .not.toBe("missing");
  const agentItem = (await agentItemByMemberId.isVisible().catch(() => false))
    ? agentItemByMemberId
    : agentItemByLabel;
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
  await expect(
    page.getByRole("menuitem", { name: TEAM_ADD_AGENT_ENTRY_LABEL_PATTERN })
  ).toBeVisible();
}

function teamForgeDialog(page: import("@playwright/test").Page) {
  return page
    .locator("[role='dialog']")
    .filter({ has: page.getByLabel(TEAM_MEMBER_NAME_LABEL_PATTERN) })
    .last();
}

async function visibleTaskDetailSurface(page: Page): Promise<Locator | null> {
  const taskDetailDialog = page.getByRole("dialog", { name: "Task detail" });
  if (await taskDetailDialog.isVisible().catch(() => false)) {
    return taskDetailDialog;
  }

  const taskDetailDock = page.locator(".team-task-detail-dock").first();
  if (await taskDetailDock.isVisible().catch(() => false)) {
    return taskDetailDock;
  }

  return null;
}

export async function openKanbanDeveloperTools(page: Page): Promise<Locator> {
  let taskDetailSurface = await visibleTaskDetailSurface(page);
  if (!taskDetailSurface) {
    const firstTaskCard = page.locator('[data-team-surface="kanban"] .team-item').first();
    await expect(firstTaskCard).toBeVisible();
    await firstTaskCard.click();

    taskDetailSurface = await visibleTaskDetailSurface(page);
    if (!taskDetailSurface) {
      throw new Error("Task detail surface did not open");
    }
  }

  const compilePreviewButton = taskDetailSurface.getByRole("button", {
    name: "Compile Preview",
    exact: true,
  });
  const compilePreviewVisible = await compilePreviewButton.isVisible().catch(() => false);
  if (!compilePreviewVisible) {
    const developerToolsSummary = taskDetailSurface.locator("summary").filter({
      has: page.getByText("Developer tools", { exact: true }),
    });
    await expect(developerToolsSummary).toBeVisible();
    await developerToolsSummary.click();
  }
  await expect(compilePreviewButton).toBeVisible();
  await expect(compilePreviewButton).toBeEnabled();
  return compilePreviewButton;
}

async function closeTaskDetailModalIfOpen(
  page: import("@playwright/test").Page
): Promise<void> {
  const taskDetailDialog = page.getByRole("dialog", { name: "Task detail" });
  if (!(await taskDetailDialog.isVisible().catch(() => false))) {
    return;
  }

  const closeButton = taskDetailDialog.locator(".mantine-Modal-close").first();
  if (await closeButton.isVisible().catch(() => false)) {
    await closeButton.click();
    await expect(taskDetailDialog).toBeHidden();
    return;
  }

  await page.keyboard.press("Escape").catch(() => {});
  await expect(taskDetailDialog).toBeHidden();
}

export async function selectPrimaryTeamEntryFromSidebar(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  const sidebar = page.locator(".teams-sidebar");
  let entry = sidebar.locator("button", { hasText: label }).first();
  if ((await entry.count()) === 0 && label === "Kanban") {
    await revealTeamSidebarSubject(page, "tasks");
    entry = sidebar.locator("button", { hasText: label }).first();
  }
  await expect(entry).toBeVisible();
  await entry.click();
}

async function revealTeamSidebarSubject(
  page: import("@playwright/test").Page,
  subject: "channels" | "tasks" | "agents" | "search"
): Promise<void> {
  const sidebar = page.locator(".teams-sidebar");
  const subjectButtonName =
    subject === "channels"
      ? "Show channels"
      : subject === "tasks"
        ? "Show tasks"
        : subject === "agents"
          ? "Show agents"
          : "Show search";
  const subjectButton = sidebar.getByRole("button", { name: subjectButtonName }).first();
  if (await subjectButton.isVisible().catch(() => false)) {
    await subjectButton.click();
    return;
  }

  const teamId = currentTeamId(page);
  if (subject === "search") {
    await pushTeamPath(page, buildTeamSearchCompatibilityPath(teamId));
    return;
  }
  const lens: WorkspaceLens =
    subject === "agents" ? "members" : subject === "tasks" ? "tasks" : "channels";
  await pushTeamPath(
    page,
    lens === "channels" ? buildTeamChannelPath(teamId) : buildTeamLensNavigationPath(teamId, lens)
  );
}

async function restoreTeamChannelWorkspace(
  page: import("@playwright/test").Page
): Promise<boolean> {
  await revealTeamSidebarSubject(page, "channels");
  const sidebarConversationEntry = teamChannelSidebarEntry(page, "all");
  const isVisible = await sidebarConversationEntry.isVisible().catch(() => false);
  if (!isVisible) {
    await navigateToTeamChannelWorkspace(page, "all");
  }
  if (!(await sidebarConversationEntry.isVisible().catch(() => false))) {
    return false;
  }
  await expect(sidebarConversationEntry).toBeVisible();
  await sidebarConversationEntry.click();
  return true;
}

export async function selectTeamChannelFromSidebar(
  page: import("@playwright/test").Page,
  channelId: string
): Promise<void> {
  await revealTeamSidebarSubject(page, "channels");
  const channelEntry = teamChannelSidebarEntry(page, channelId);
  if (!(await channelEntry.isVisible().catch(() => false))) {
    await navigateToTeamChannelWorkspace(page, channelId);
  }
  await expect(channelEntry).toBeVisible();
  await channelEntry.click();
}

export function teamChannelSidebarEntry(
  page: import("@playwright/test").Page,
  channelId: string
) {
  assertRawChannelId(channelId);
  const channelNamePattern = new RegExp(
    `#\\s*${escapeRegExp(channelId)}(?:\\s|$)`
  );
  return page
    .locator(".teams-sidebar")
    .getByRole("button", { name: channelNamePattern })
    .first();
}

function assertRawChannelId(channelId: string): void {
  if (channelId.trim().startsWith("#")) {
    throw new Error(
      `Expected a raw channel id without the display prefix, got ${JSON.stringify(channelId)}`
    );
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function navigateToTeamChannelWorkspace(
  page: import("@playwright/test").Page,
  channelId: string
): Promise<void> {
  assertRawChannelId(channelId);
  await pushTeamPath(
    page,
    buildTeamChannelPath(currentTeamId(page), channelId)
  );
}

function currentTeamId(page: import("@playwright/test").Page): string {
  const currentUrl = new URL(page.url());
  const route = resolveTeamRoute(currentUrl.pathname);
  if (!route || route.mode !== "detail") {
    throw new Error(`Expected a Team detail route, got ${currentUrl.pathname}`);
  }
  return route.teamId;
}

async function pushTeamPath(
  page: import("@playwright/test").Page,
  path: string
): Promise<void> {
  await page.evaluate((nextPath) => {
    window.history.pushState({}, "", nextPath);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, path);
}

async function navigateToTeamTab(
  page: import("@playwright/test").Page,
  tab: Parameters<typeof buildTeamTabCompatibilityPath>[1]
): Promise<void> {
  const currentUrl = new URL(page.url());
  await pushTeamPath(page, buildTeamTabCompatibilityPath(currentUrl.pathname, tab));
}

export async function openMainTeamAction(
  page: import("@playwright/test").Page,
  label: string,
  allowSidebarReset = true
): Promise<void> {
  await closeTaskDetailModalIfOpen(page);

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
    if (await restoreTeamChannelWorkspace(page)) {
      await openMainTeamAction(page, label, false);
      return;
    }
  }

  if (label === "Execution Runs") {
    await navigateToTeamTab(page, "runs");
    await expect(page.locator(".teams-main")).toContainText("Execution Runs");
    return;
  }

  throw new Error(`Team action not found: ${label}`);
}

export async function openAdvancedView(
  page: import("@playwright/test").Page,
  label: string,
  allowSidebarReset = true
): Promise<void> {
  await closeTaskDetailModalIfOpen(page);
  const trigger = page
    .getByRole("button", { name: /^(More|Open more workspace actions)$/ })
    .first();
  if ((await trigger.count()) === 0 && allowSidebarReset) {
    if (await restoreTeamChannelWorkspace(page)) {
      await openAdvancedView(page, label, false);
      return;
    }
  }
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
