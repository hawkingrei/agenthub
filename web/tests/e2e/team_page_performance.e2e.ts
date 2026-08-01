import { expect, test } from "./coverage";
import {
  enableDeveloperMode,
  gotoTeams,
  jsonResponse,
  mockTeamPageApis,
  openAdvancedView,
  openTeamFromSelector,
  selectAgentFromSidebar,
  selectTeamChannelFromSidebar,
  type TeamActorMessageRecord,
  type TeamRunRecord,
} from "./team_page_helpers";
import type { TeamConversationMessageRecord } from "./team_page_fixture";

function buildChannelMessages(params: {
  teamId: string;
  channelId: string;
  now: number;
  count: number;
}): TeamConversationMessageRecord[] {
  const taskId = `task-${params.teamId}-channel-${params.channelId}`;
  return Array.from({ length: params.count }, (_, index) => {
    const messageId = index + 1;
    return {
      message_id: messageId,
      conversation_id: `conversation-${taskId}`,
      task_id: taskId,
      from_actor_id: messageId % 2 === 0 ? "agent-worker-1" : "agent-coordinator-1",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "chat_message",
        text: `Long channel browser item ${messageId}`,
      },
      created_at: params.now + messageId,
    };
  });
}

function buildAcpAgentEvents(params: {
  agentId: string;
  sessionId: string;
  now: number;
  count: number;
}) {
  return Array.from({ length: params.count }, (_, index) => {
    const eventId = index + 1;
    return {
      event_id: eventId,
      agent_id: params.agentId,
      session_id: params.sessionId,
      seq: String(eventId),
      ts: params.now + eventId,
      stream: "acp",
      message: JSON.stringify({
        type: "agent_message",
        text: `ACP browser item ${eventId}`,
      }),
    };
  });
}

function buildMailboxMessages(params: {
  runId: string;
  now: number;
  count: number;
}): TeamActorMessageRecord[] {
  return Array.from({ length: params.count }, (_, index) => {
    const messageId = index + 1;
    return {
      message_id: messageId,
      run_id: params.runId,
      from_actor_id: messageId % 2 === 0 ? "agent-coordinator-1" : "agent-worker-1",
      to_actor_id: messageId % 2 === 0 ? "agent-worker-1" : "agent-coordinator-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "chat_message",
        text: `Mailbox browser item ${messageId}`,
      },
      status: "pending",
      created_at: params.now + messageId,
      delivered_at: null,
    };
  });
}

test("team and ACP heavy browser surfaces keep initial long histories windowed", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
  });
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-browser-perf";
  const channelId = "perf";
  const channelTaskId = `task-${teamId}-channel-${channelId}`;
  const memberId = "agent-worker-1";
  const memberSessionId = `session-${teamId}-${memberId}`;
  const runId = `${teamId}-working-1`;
  const runRecord: TeamRunRecord = {
    id: runId,
    team_id: teamId,
    context_id: `ctx-${runId}`,
    status: "working",
    input: { prompt: "browser performance mailbox" },
    created_at: fixture.now + 500,
    started_at: fixture.now + 501,
    ended_at: null,
  };
  const acpEvents = buildAcpAgentEvents({
    agentId: memberId,
    sessionId: memberSessionId,
    now: fixture.now + 1_000,
    count: 180,
  });

  fixture.teams.push({
    id: teamId,
    name: "Browser Perf Team",
    description: "browser performance fixture",
    spec: {
      coordinator_member_id: "agent-coordinator-1",
      members: [
        { member_id: "agent-coordinator-1", role: "coordinator", model: "codex" },
        { member_id: memberId, role: "worker", model: "gemini" },
      ],
      steps: [
        { step_key: "coordinator_plan", member_id: "agent-coordinator-1" },
        { step_key: "worker_execute", member_id: memberId },
      ],
    },
    created_at: fixture.now + 10,
    updated_at: fixture.now + 10,
  });
  fixture.seedMailboxMessages(
    runId,
    buildMailboxMessages({
      runId,
      now: fixture.now + 3_000,
      count: 90,
    })
  );

  await page.route(new RegExp(`/api/teams/${teamId}/runs(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([runRecord]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(runRecord));
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const agentId = decodeURIComponent(
      request.url().match(/\/api\/agents\/([^/]+)\/events/)?.[1] ?? ""
    );
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(agentId === memberId ? acpEvents : []),
    });
  });

  await enableDeveloperMode(page);
  await page.setViewportSize({ width: 1280, height: 800 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Browser Perf Team");
  await page.evaluate(() => performance.mark("team-channel-open-start"));
  await page.getByLabel("Create channel").click();
  await page.getByLabel("Channel ID").fill(channelId);
  await page.getByLabel("Channel Description").fill("Long browser performance channel");
  await page.locator('button[type="submit"]').filter({ hasText: "Create channel" }).click();
  fixture.seedTaskMessages(
    channelTaskId,
    buildChannelMessages({
      teamId,
      channelId,
      now: fixture.now + 2_000,
      count: 160,
    })
  );
  await selectTeamChannelFromSidebar(page, channelId);
  await expect(page.getByText("Long channel browser item 160")).toBeVisible();
  await page.evaluate(() => {
    performance.mark("team-channel-open-visible");
    performance.measure(
      "team-channel-open",
      "team-channel-open-start",
      "team-channel-open-visible"
    );
  });

  const channelMetrics = await page.evaluate(() => {
    const renderedRowNodes = Array.from(
      document.querySelectorAll<HTMLElement>("[data-team-channel-item='true']")
    );
    const renderedRowText = renderedRowNodes.map((node) => node.textContent ?? "").join("\n");
    const oldestVisible = /Long channel browser item 1(?!\d)/.test(renderedRowText);
    const latestVisible = renderedRowText.includes("Long channel browser item 160");
    const measure = performance.getEntriesByName("team-channel-open").at(-1);
    return {
      renderedRows: renderedRowNodes.length,
      oldestVisible,
      latestVisible,
      renderMs: measure ? Math.round(measure.duration) : null,
    };
  });

  expect(channelMetrics.latestVisible).toBe(true);
  expect(channelMetrics.oldestVisible).toBe(false);
  expect(channelMetrics.renderedRows).toBeGreaterThan(0);
  expect(channelMetrics.renderedRows).toBeLessThan(40);
  expect(channelMetrics.renderMs).not.toBeNull();
  expect(channelMetrics.renderMs).toBeGreaterThanOrEqual(0);

  const channelScroll = page.locator("[data-team-channel-scroll='true']");
  await channelScroll.evaluate((node) => {
    node.scrollTop = node.scrollHeight;
    node.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await page.waitForFunction(() =>
    Boolean(document.querySelector("[data-team-channel-top-spacer='true']"))
  );
  await channelScroll.evaluate((node) => {
    node.scrollTop = 0;
    node.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await expect(page.getByLabel("Jump to bottom")).toBeVisible();
  await expect(page.getByText("Long channel browser item 1", { exact: true })).toBeVisible();
  await expect(page.getByText("Long channel browser item 160", { exact: true })).toHaveCount(1);

  await page.evaluate(() => performance.mark("team-acp-open-start"));
  await selectAgentFromSidebar(page, "Worker Agent");
  await expect(page.getByText("ACP browser item 180")).toBeVisible();
  await page.evaluate(() => {
    performance.mark("team-acp-open-visible");
    performance.measure("team-acp-open", "team-acp-open-start", "team-acp-open-visible");
  });

  const acpMetrics = await page.evaluate(() => {
    const renderedRowNodes = Array.from(
      document.querySelectorAll<HTMLElement>(".acp-conversation-item")
    );
    const hasSpacer = Boolean(document.querySelector(".acp-conversation-spacer"));
    const renderedRowText = renderedRowNodes.map((node) => node.textContent ?? "").join("\n");
    const oldestVisible = /ACP browser item 1(?!\d)/.test(renderedRowText);
    const latestVisible = renderedRowText.includes("ACP browser item 180");
    const measure = performance.getEntriesByName("team-acp-open").at(-1);
    return {
      renderedRows: renderedRowNodes.length,
      hasSpacer,
      oldestVisible,
      latestVisible,
      renderMs: measure ? Math.round(measure.duration) : null,
    };
  });

  expect(acpMetrics.latestVisible).toBe(true);
  expect(acpMetrics.oldestVisible).toBe(false);
  expect(acpMetrics.hasSpacer).toBe(false);
  expect(acpMetrics.renderedRows).toBeGreaterThan(0);
  expect(acpMetrics.renderedRows).toBeLessThan(40);
  expect(acpMetrics.renderMs).not.toBeNull();
  expect(acpMetrics.renderMs).toBeGreaterThanOrEqual(0);

  await page.evaluate(() => performance.mark("team-mailbox-open-start"));
  await openAdvancedView(page, "Execution Mailbox");
  await page
    .locator(".teams-chat-members .team-item", { hasText: "Worker Agent (worker)" })
    .click();
  await expect(page.getByText("Mailbox browser item 90")).toBeVisible();
  await page.evaluate(() => {
    performance.mark("team-mailbox-open-visible");
    performance.measure(
      "team-mailbox-open",
      "team-mailbox-open-start",
      "team-mailbox-open-visible"
    );
  });

  const mailboxMetrics = await page.evaluate(() => {
    const renderedRowNodes = Array.from(
      document.querySelectorAll<HTMLElement>("[data-team-mailbox-message-id]")
    );
    const renderedRowText = renderedRowNodes.map((node) => node.textContent ?? "").join("\n");
    const oldestVisible = /Mailbox browser item 1(?!\d)/.test(renderedRowText);
    const latestVisible = renderedRowText.includes("Mailbox browser item 90");
    const measure = performance.getEntriesByName("team-mailbox-open").at(-1);
    return {
      renderedRows: renderedRowNodes.length,
      oldestVisible,
      latestVisible,
      renderMs: measure ? Math.round(measure.duration) : null,
    };
  });

  expect(mailboxMetrics.latestVisible).toBe(true);
  expect(mailboxMetrics.oldestVisible).toBe(false);
  expect(mailboxMetrics.renderedRows).toBeGreaterThan(0);
  expect(mailboxMetrics.renderedRows).toBeLessThan(40);
  expect(mailboxMetrics.renderMs).not.toBeNull();
  expect(mailboxMetrics.renderMs).toBeGreaterThanOrEqual(0);
  expect(pageErrors).toEqual([]);
});
