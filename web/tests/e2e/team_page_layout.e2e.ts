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
  const channelId = "layout";
  const channelTaskId = `task-${teamId}-channel-${channelId}`;
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
  fixture.seedTaskMessages(channelTaskId, [
    {
      message_id: 101,
      conversation_id: `conversation-${channelTaskId}`,
      task_id: channelTaskId,
      from_actor_id: "planner",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "chat_message",
        text: "Layout root update for ratio test.",
      },
      created_at: fixture.now + 95,
    },
    {
      message_id: 102,
      conversation_id: `conversation-${channelTaskId}`,
      task_id: channelTaskId,
      from_actor_id: "worker-1",
      to_actor_id: null,
      route: "team_thread_reply",
      payload: {
        type: "chat_message",
        text: "Layout thread reply for ratio test.",
        thread_root_message_id: 101,
      },
      created_at: fixture.now + 96,
    },
  ]);

  await page.setViewportSize({ width: 1280, height: 800 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Layout Team");
  await page.getByLabel("Create channel").click();
  await page.getByLabel("Channel ID").fill(channelId);
  await page.getByLabel("Channel Description").fill("Layout ratio channel");
  await page.locator('button[type="submit"]').filter({ hasText: "Create channel" }).click();
  await selectTeamChannelFromSidebar(page, channelId);
  await expect(page.locator('[data-team-workspace-header-shell="true"]')).toBeVisible();
  await expect(page.locator('[data-team-surface="channel-thread-layout"]')).toHaveAttribute(
    "data-thread-open",
    "false"
  );

  const metrics = await page.evaluate(() => {
    const layout = document.querySelector<HTMLElement>(".teams-layout");
    const sidebar = document.querySelector<HTMLElement>('[data-team-surface="sidebar"]');
    const workbench = document.querySelector<HTMLElement>('[data-team-surface="workbench"]');
    const channelThreadLayout = document.querySelector<HTMLElement>(
      '[data-team-surface="channel-thread-layout"]'
    );
    const channelPane = document.querySelector<HTMLElement>('[data-team-surface="channel-pane"]');
    const headerShell = document.querySelector<HTMLElement>(
      '[data-team-workspace-header-shell="true"]'
    );
    const sidebarSubjectTabs = Array.from(
      document.querySelectorAll<HTMLElement>('[role="tab"]')
    ).filter((node) => /^Show (channels|tasks|agents|search)$/i.test(node.getAttribute("aria-label") ?? ""));
    const sidebarSubjectTablist = document.querySelector<HTMLElement>(
      '[role="tablist"][aria-label="Team sidebar sections"]'
    );
    if (!layout || !sidebar || !workbench || !headerShell || !channelThreadLayout || !channelPane) {
      throw new Error("team workspace layout nodes missing");
    }
    const layoutRect = layout.getBoundingClientRect();
    const sidebarRect = sidebar.getBoundingClientRect();
    const workbenchRect = workbench.getBoundingClientRect();
    const channelThreadLayoutRect = channelThreadLayout.getBoundingClientRect();
    const channelPaneRect = channelPane.getBoundingClientRect();
    const headerRect = headerShell.getBoundingClientRect();
    return {
      layoutClassName: layout.className,
      sidebarWidth: sidebarRect.width,
      workbenchWidth: workbenchRect.width,
      layoutWidth: layoutRect.width,
      channelThreadLayoutWidth: channelThreadLayoutRect.width,
      channelPaneWidth: channelPaneRect.width,
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
  expect(metrics.channelPaneWidth / metrics.channelThreadLayoutWidth).toBeGreaterThanOrEqual(0.98);
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
  await page.keyboard.press("Escape");
  await expect(searchInput).toBeHidden();

  await page.evaluate(
    ({ nextTeamId, nextChannelId, nextTaskId }) => {
      const nextUrl = new URL(window.location.href);
      nextUrl.pathname = `/workspace/teams/${nextTeamId}`;
      nextUrl.search = "";
      nextUrl.searchParams.set("channel", nextChannelId);
      nextUrl.searchParams.set("task", nextTaskId);
      nextUrl.searchParams.set("thread", "101");
      window.history.pushState({}, "", `${nextUrl.pathname}${nextUrl.search}`);
      window.dispatchEvent(new PopStateEvent("popstate"));
    },
    { nextTeamId: teamId, nextChannelId: channelId, nextTaskId: channelTaskId }
  );
  await expect(page.locator('[data-team-surface="thread-pane"]')).toBeVisible();
  await expect(page.locator('[data-team-surface="channel-thread-layout"]')).toHaveAttribute(
    "data-thread-open",
    "true"
  );

  const splitMetrics = await page.evaluate(() => {
    const layout = document.querySelector<HTMLElement>(
      '[data-team-surface="channel-thread-layout"]'
    );
    const channelPane = document.querySelector<HTMLElement>('[data-team-surface="channel-pane"]');
    const threadDock = document.querySelector<HTMLElement>('[data-team-surface="thread-dock"]');
    if (!layout || !channelPane || !threadDock) {
      throw new Error("team workspace split nodes missing");
    }
    const layoutRect = layout.getBoundingClientRect();
    const channelRect = channelPane.getBoundingClientRect();
    const threadRect = threadDock.getBoundingClientRect();
    return {
      layoutWidth: layoutRect.width,
      channelWidth: channelRect.width,
      threadWidth: threadRect.width,
      horizontalGap: threadRect.left - channelRect.right,
      overlaps: channelRect.right > threadRect.left,
      threadRightInset: layoutRect.right - threadRect.right,
    };
  });
  expect(splitMetrics.overlaps).toBe(false);
  expect(splitMetrics.horizontalGap).toBeGreaterThanOrEqual(0);
  expect(splitMetrics.threadRightInset).toBeGreaterThanOrEqual(-1);
  expect(splitMetrics.channelWidth / splitMetrics.layoutWidth).toBeGreaterThanOrEqual(0.48);
  expect(splitMetrics.channelWidth / splitMetrics.layoutWidth).toBeLessThanOrEqual(0.58);
  expect(splitMetrics.threadWidth / splitMetrics.layoutWidth).toBeGreaterThanOrEqual(0.40);
  expect(splitMetrics.threadWidth / splitMetrics.layoutWidth).toBeLessThanOrEqual(0.50);

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('[data-team-surface="thread-pane"]')).toBeVisible();
  const compactMetrics = await page.evaluate(() => {
    const visibleSurfaceRects = Array.from(
      document.querySelectorAll<HTMLElement>(
        '[data-team-surface="sidebar"], [data-team-surface="workbench"], [data-team-surface="channel-pane"], [data-team-surface="thread-dock"]'
      )
    )
      .filter((node) => {
        const style = window.getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0;
      })
      .map((node) => {
        const rect = node.getBoundingClientRect();
        return {
          surface: node.dataset.teamSurface ?? "",
          left: rect.left,
          right: rect.right,
          top: rect.top,
          bottom: rect.bottom,
          width: rect.width,
          height: rect.height,
        };
      });
    const horizontalOverflow =
      document.documentElement.scrollWidth - document.documentElement.clientWidth;
    const badHorizontalBounds = visibleSurfaceRects.filter(
      (rect) => rect.left < -1 || rect.right > document.documentElement.clientWidth + 1
    );
    const overlappingPairs: string[] = [];
    for (let index = 0; index < visibleSurfaceRects.length; index += 1) {
      for (let next = index + 1; next < visibleSurfaceRects.length; next += 1) {
        const left = visibleSurfaceRects[index];
        const right = visibleSurfaceRects[next];
        const intersects =
          left.left < right.right &&
          left.right > right.left &&
          left.top < right.bottom &&
          left.bottom > right.top;
        const nested =
          (left.left >= right.left &&
            left.right <= right.right &&
            left.top >= right.top &&
            left.bottom <= right.bottom) ||
          (right.left >= left.left &&
            right.right <= left.right &&
            right.top >= left.top &&
            right.bottom <= left.bottom);
        if (intersects && !nested) {
          overlappingPairs.push(`${left.surface}:${right.surface}`);
        }
      }
    }
    return {
      horizontalOverflow,
      badHorizontalBounds,
      overlappingPairs,
    };
  });
  expect(compactMetrics.horizontalOverflow).toBeLessThanOrEqual(1);
  expect(compactMetrics.badHorizontalBounds).toEqual([]);
  expect(compactMetrics.overlappingPairs).toEqual([]);
});
