import { expect, test } from "./coverage";
import {
  gotoTeams,
  mockTeamPageApis,
  openTeamFromSelector,
  selectTeamChannelFromSidebar,
} from "./team_page_helpers";

test("team workspace keeps desktop layout proportions across shell, sidebar, header, and lenses", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-layout";
  const teamCreatedAt = fixture.now + 90;
  fixture.teams.push({
    id: teamId,
    name: "Layout Team",
    description: "layout contract e2e",
    spec: {
      coordinator_member_id: "planner",
      members: [
        { member_id: "planner", role: "coordinator", model: "codex" },
        { member_id: "worker-1", role: "worker", model: "codex" },
      ],
      steps: [
        { step_key: "coordinator_plan" },
        { step_key: "worker_execute", member_id: "worker-1" },
      ],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await page.setViewportSize({ width: 1280, height: 800 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Layout Team");
  await selectTeamChannelFromSidebar(page, "all");
  await expect(page.locator('[data-team-workspace-header-shell="true"]')).toBeVisible();

  const metrics = await page.evaluate(() => {
    const layout = document.querySelector<HTMLElement>(".teams-layout");
    const sidebar = document.querySelector<HTMLElement>('[data-team-surface="sidebar"]');
    const workbench = document.querySelector<HTMLElement>('[data-team-surface="workbench"]');
    const headerShell = document.querySelector<HTMLElement>(
      '[data-team-workspace-header-shell="true"]'
    );
    const sidebarSubjectTabs = Array.from(
      document.querySelectorAll<HTMLElement>('[role="tab"]')
    ).filter((node) => /^Show (channels|tasks|agents|search)$/i.test(node.getAttribute("aria-label") ?? ""));
    const sidebarSubjectTablist = document.querySelector<HTMLElement>(
      '[role="tablist"][aria-label="Team sidebar sections"]'
    );
    if (!layout || !sidebar || !workbench || !headerShell) {
      throw new Error("team workspace layout nodes missing");
    }
    const layoutRect = layout.getBoundingClientRect();
    const sidebarRect = sidebar.getBoundingClientRect();
    const workbenchRect = workbench.getBoundingClientRect();
    const headerRect = headerShell.getBoundingClientRect();
    return {
      layoutClassName: layout.className,
      sidebarWidth: sidebarRect.width,
      workbenchWidth: workbenchRect.width,
      layoutWidth: layoutRect.width,
      headerHeight: headerRect.height,
      workbenchHeight: workbenchRect.height,
      headerClassName: headerShell.className,
      sidebarSubjectTablistClassName: sidebarSubjectTablist?.className ?? "",
      sidebarSubjectTabs: sidebarSubjectTabs.map((node) => ({
        label: node.getAttribute("aria-label") ?? "",
        clientWidth: node.clientWidth,
        scrollWidth: node.scrollWidth,
        className: node.className,
      })),
    };
  });

  expect(metrics.layoutClassName).toContain(
    "grid-cols-[var(--teams-sidebar-width,260px)_1fr]"
  );
  expect(metrics.sidebarWidth).toBeGreaterThanOrEqual(240);
  expect(metrics.sidebarWidth).toBeLessThanOrEqual(300);
  expect(metrics.workbenchWidth / metrics.sidebarWidth).toBeGreaterThanOrEqual(3.3);
  expect(metrics.workbenchWidth / metrics.sidebarWidth).toBeLessThanOrEqual(4.3);
  expect(metrics.headerHeight).toBeLessThanOrEqual(74);
  expect(metrics.headerHeight / metrics.workbenchHeight).toBeLessThanOrEqual(0.12);
  expect(metrics.headerClassName).not.toContain("teams-panel-card");
  expect(metrics.sidebarSubjectTablistClassName).toContain("items-center");
  expect(metrics.sidebarSubjectTabs).toHaveLength(4);
  for (const tab of metrics.sidebarSubjectTabs) {
    expect(tab.scrollWidth, tab.label).toBeLessThanOrEqual(tab.clientWidth);
    expect(tab.className).toContain("justify-center");
  }

  await page.getByRole("tab", { name: "Show search", exact: true }).click();
  const searchInput = page.getByPlaceholder("Search channels, tasks, or agents");
  await expect(searchInput).toBeVisible();
  await expect(searchInput).toHaveAttribute(
    "placeholder",
    "Search channels, tasks, or agents"
  );
});
