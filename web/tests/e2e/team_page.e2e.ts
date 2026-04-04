import { expect, test, testLocalLlm } from "./coverage";
import { UI_PREFS_STORAGE_KEY } from "../../src/ui/developer_mode";

type StoredAuthState = {
  token: string;
  userId: string;
  username: string;
  role: string;
};

type E2eAgentRecord = {
  id: string;
  name: string;
  workdir: string;
  command: string;
  args: string[];
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  status: string;
  created_at: number;
  updated_at: number;
};

type TeamSpecMember = {
  member_id: string;
  role?: string;
  description?: string;
  model?: string;
  skills?: string[];
};

type TeamSpecStep = {
  step_key: string;
  member_id?: string;
  depends_on?: string[];
};

type TeamSpecPayload = {
  spec_version?: number;
  entrypoint?: string;
  leader_member_id?: string;
  members: TeamSpecMember[];
  steps?: TeamSpecStep[];
};

type CreateTeamPayload = {
  name: string;
  description?: string;
  spec: TeamSpecPayload;
};

type UpdateTeamSpecPayload = {
  spec: TeamSpecPayload;
  expected_updated_at: number;
};

type TeamDefinitionRecord = {
  id: string;
  name: string;
  description?: string | null;
  spec: TeamSpecPayload;
  created_at: number;
  updated_at: number;
};

type TeamRunRecord = {
  id: string;
  team_id: string;
  context_id: string;
  status:
    | "submitted"
    | "working"
    | "input_required"
    | "completed"
    | "failed"
    | "canceled";
  input: Record<string, unknown>;
  created_at: number;
  started_at: number | null;
  ended_at: number | null;
};

type TeamRuntimeStatus = "running" | "stopped" | "degraded";

type MockTeamRuntimeState = {
  status: TeamRuntimeStatus;
};

type TeamTaskRecord = {
  id: string;
  team_id: string;
  title: string;
  status: "open" | "in_progress" | "completed" | "archived";
  created_by_actor_id: string;
  context: Record<string, unknown>;
  created_at: number;
  updated_at: number;
};

type TeamConversationMessageRecord = {
  message_id: number;
  conversation_id: string;
  task_id: string;
  from_actor_id: string;
  to_actor_id: string | null;
  route: "to_leader" | "to_member" | "group_chat";
  payload: unknown;
  created_at: number;
};

type TeamActorMessageRecord = {
  message_id: number;
  run_id: string;
  from_actor_id: string;
  to_actor_id: string;
  channel: string;
  transport: "local" | "remote";
  route: Record<string, unknown> | null;
  payload: unknown;
  status: "pending" | "delivered" | "dead_letter";
  created_at: number;
  delivered_at: number | null;
};

type TeamPageFixture = {
  now: number;
  auth: StoredAuthState;
  agents: E2eAgentRecord[];
  teams: TeamDefinitionRecord[];
  getCreatePayload: () => CreateTeamPayload | null;
  getUpdateSpecPayloads: () => Array<{ teamId: string; payload: UpdateTeamSpecPayload }>;
};

function jsonResponse(data: unknown, status = 200): {
  status: number;
  contentType: string;
  body: string;
} {
  return {
    status,
    contentType: "application/json",
    body: JSON.stringify(data),
  };
}

function buildTeamRun(
  teamId: string,
  status: TeamRunRecord["status"],
  createdAt: number,
  index: number
): TeamRunRecord {
  return {
    id: `${teamId}-${status}-${index}`,
    team_id: teamId,
    context_id: `ctx-${teamId}-${index}`,
    status,
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}

async function createTeamFromModal(
  page: import("@playwright/test").Page,
  options: {
    name: string;
    goal?: string;
  }
): Promise<void> {
  await openMainTeamAction(page, "Create Team");
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
  await expect(page).toHaveURL(/\/teams\/[^/?#]+(?:[?#].*)?$/);
  await expect.poll(() => isTeamDetailReady(page)).toBe(true);
  await expectAddAgentEntryVisible(page, options.name);
}

type AddAgentEntryLane = "primary" | "menuItem" | "menuTrigger";

async function restoreTeamAddAgentContext(
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

async function waitForAddAgentEntryLane(
  page: import("@playwright/test").Page,
  teamName?: string
): Promise<AddAgentEntryLane> {
  const primaryButton = page
    .locator(".teams-main")
    .getByRole("button", { name: "Add Agent", exact: true })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", { name: "Add Agent", exact: true });
  const menuTrigger = page.getByRole("button", {
    name: "Open selected team menu",
    exact: true,
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

async function createTeamMemberFromModal(
  page: import("@playwright/test").Page,
  options: {
    teamName?: string;
    workdir: string;
    model?: string;
    identity?: string;
  }
): Promise<void> {
  const roleModelLabels: Record<string, string> = {
    codex: "Codex ACP",
    gemini: "Gemini CLI",
    kimi: "Kimi CLI",
  };
  const openButtonLabel = "Add Agent";
  const confirmLabel = "Create Agent";
  const primaryOpenButton = page
    .locator(".teams-main")
    .getByRole("button", { name: openButtonLabel, exact: true })
    .first();
  const visibleMenuItem = page.getByRole("menuitem", { name: openButtonLabel, exact: true });
  const menuTrigger = page.getByRole("button", {
    name: "Open selected team menu",
    exact: true,
  });
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
  if (options.identity) {
    await dialog.getByLabel("Identity").fill(options.identity);
  }
  if (options.model) {
    const optionLabel = roleModelLabels[options.model] ?? options.model;
    await dialog.getByLabel("Role model").click();
    await page.getByRole("option", { name: optionLabel, exact: true }).click();
  }
  await dialog.getByLabel(/Workdir/).fill(options.workdir);
  await dialog.getByRole("button", { name: confirmLabel }).click();
  await expect(dialog).toBeHidden();
}

async function openTeamFromSelector(
  page: import("@playwright/test").Page,
  teamName: string
): Promise<void> {
  const pathname = new URL(page.url()).pathname;
  if (pathname !== "/teams" && pathname !== "/teams/") {
    const selectorButton = page.getByRole("button", { name: "Team Selector", exact: true });
    if ((await selectorButton.count()) > 0) {
      await selectorButton.first().click();
    } else {
      await page.goto("/teams", { waitUntil: "domcontentloaded" });
    }
  }
  await expect(page).toHaveURL(/\/teams(?:[/?#]|$)/);
  const teamItem = page.locator(".team-item", { hasText: teamName }).first();
  await expect(teamItem).toBeVisible();
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await teamItem.scrollIntoViewIfNeeded();
    try {
      await teamItem.click({ timeout: 1_500, force: attempt > 0 });
    } catch {
      await teamItem.click({ force: true });
    }
    await page.waitForTimeout(150);
    if (await isTeamDetailReady(page)) {
      return;
    }
  }
  await teamItem.evaluate((element) => {
    (element as HTMLButtonElement).click();
  });
  await expect(page).toHaveURL(/\/teams\/.+/);
  await expect.poll(() => isTeamDetailReady(page)).toBe(true);
}

async function isTeamDetailReady(page: import("@playwright/test").Page): Promise<boolean> {
  const detailPath = new URL(page.url()).pathname;
  if (!/^\/teams\/[^/]+$/.test(detailPath)) {
    return false;
  }
  const pageText = await page.locator("body").textContent();
  if (!pageText) {
    return false;
  }
  return (
    !pageText.includes("Loading team workspace...") &&
    !pageText.includes("This team is unavailable.")
  );
}

async function gotoTeams(page: import("@playwright/test").Page): Promise<void> {
  await page.goto("/teams", { waitUntil: "domcontentloaded" });
  await expect(page.getByRole("heading", { name: "Team Selector" })).toBeVisible();
}

async function selectAgentFromSidebar(
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

async function expectTeamRuntimeBadge(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  await openSelectedTeamMenu(page);
  await expect(page.getByRole("menu").getByText(label).first()).toBeVisible();
}

async function openSelectedTeamMenu(
  page: import("@playwright/test").Page
): Promise<void> {
  const trigger = page.getByRole("button", { name: "Open selected team menu", exact: true });
  await expect(trigger).toBeVisible();
  if ((await trigger.getAttribute("aria-expanded")) !== "true") {
    await trigger.click();
  }
}

async function clickSelectedTeamMenuItem(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  await openSelectedTeamMenu(page);
  const item = page.getByRole("menuitem", { name: label, exact: true });
  await expect(item).toBeVisible();
  await item.click();
}

async function expectAddAgentEntryVisible(
  page: import("@playwright/test").Page,
  teamName?: string
): Promise<void> {
  const lane = await waitForAddAgentEntryLane(page, teamName);
  if (lane === "primary" || lane === "menuItem") {
    return;
  }
  const menuTrigger = page.getByRole("button", {
    name: "Open selected team menu",
    exact: true,
  });
  await expect(menuTrigger).toBeVisible();
  await openSelectedTeamMenu(page);
  await expect(page.getByRole("menuitem", { name: "Add Agent", exact: true })).toBeVisible();
}

async function openKanbanDeveloperTools(
  page: import("@playwright/test").Page
): Promise<void> {
  const compilePreviewButton = page.getByRole("button", { name: "Compile Preview", exact: true });
  if ((await compilePreviewButton.count()) === 0) {
    const developerToolsSummary = page.locator("summary").filter({
      has: page.getByText("Developer tools", { exact: true }),
    });
    await expect(developerToolsSummary).toBeVisible();
    await developerToolsSummary.click();
  }
  await expect(compilePreviewButton).toBeVisible();
}

async function selectPrimaryTeamEntryFromSidebar(
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

async function openMainTeamAction(
  page: import("@playwright/test").Page,
  label: string,
  allowSidebarReset = true
): Promise<void> {
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

  const moreTrigger = page
    .getByRole("button", { name: "Open more workspace actions" })
    .first();
  if ((await moreTrigger.count()) > 0) {
    await expect(moreTrigger).toBeVisible();
    await moreTrigger.click();
    const menuItem = page.getByRole("menuitem", { name: label, exact: true });
    if ((await menuItem.count()) > 0) {
      await expect(menuItem).toBeVisible();
      await menuItem.click();
      return;
    }
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

async function openAdvancedView(
  page: import("@playwright/test").Page,
  label: string
): Promise<void> {
  const trigger =
    page.getByRole("button", { name: "Open more workspace actions" }).first();
  await expect(trigger).toBeVisible();
  await trigger.click();
  const menuItem = page.getByRole("menuitem", { name: label, exact: true });
  await expect(menuItem).toBeVisible();
  await menuItem.click();
}

async function enableDeveloperMode(
  page: import("@playwright/test").Page
): Promise<void> {
  await page.addInitScript((storageKey: string) => {
    window.localStorage.setItem(
      storageKey,
      JSON.stringify({ developerMode: true })
    );
  }, UI_PREFS_STORAGE_KEY);
}

async function mockTeamPageApis(
  page: import("@playwright/test").Page
): Promise<TeamPageFixture> {
  const now = 1_700_000_000;
  const auth: StoredAuthState = {
    token: "token-e2e",
    userId: "user-e2e",
    username: "e2e-user",
    role: "root",
  };
  const agents: E2eAgentRecord[] = [
    {
      id: "agent-leader-1",
      name: "Leader Agent",
      workdir: "/workspace/leader",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
    {
      id: "agent-worker-1",
      name: "Worker Agent",
      workdir: "/workspace/worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
    {
      id: "agent-worker-2",
      name: "Worker Agent Two",
      workdir: "/workspace/worker-two",
      command: "kimi",
      args: ["acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: now,
      updated_at: now,
    },
  ];
  const teams: TeamDefinitionRecord[] = [];
  const teamRuntimeStateById = new Map<string, MockTeamRuntimeState>();
  const tasksByTeamId = new Map<string, TeamTaskRecord[]>();
  const taskMessagesById = new Map<string, TeamConversationMessageRecord[]>();
  const taskCounterByTeamId = new Map<string, number>();
  const mailboxMessagesByRunId = new Map<string, TeamActorMessageRecord[]>();
  const runEventCounterByRunId = new Map<string, number>();
  let createTeamPayload: CreateTeamPayload | null = null;
  const updateSpecPayloads: Array<{ teamId: string; payload: UpdateTeamSpecPayload }> = [];

  const ensureTasks = (teamId: string): TeamTaskRecord[] => {
    const existing = tasksByTeamId.get(teamId);
    if (existing) {
      return existing;
    }
    const defaultTask: TeamTaskRecord = {
      id: `task-${teamId}-1`,
      team_id: teamId,
      title: "Default planning conversation",
      status: "open",
      created_by_actor_id: `user:${auth.userId}`,
      context: { source: "e2e-default" },
      created_at: now + 1,
      updated_at: now + 1,
    };
    tasksByTeamId.set(teamId, [defaultTask]);
    taskCounterByTeamId.set(teamId, 1);
    taskMessagesById.set(defaultTask.id, []);
    return [defaultTask];
  };

  const inferRunStatusFromRunId = (runId: string): TeamRunRecord["status"] => {
    const matched = runId.match(/-(submitted|working|input_required|completed|failed|canceled)-/);
    if (!matched) {
      return "working";
    }
    return matched[1] as TeamRunRecord["status"];
  };

  const inferTeamIdFromRunId = (runId: string): string => {
    const matchedTeam = teams.find((team) => runId.startsWith(`${team.id}-`));
    if (matchedTeam) {
      return matchedTeam.id;
    }
    const separatorIndex = runId.indexOf("-");
    if (separatorIndex > 0) {
      return runId.slice(0, separatorIndex);
    }
    return teams[0]?.id ?? "team-e2e";
  };

  const ensureMailboxMessages = (runId: string): TeamActorMessageRecord[] => {
    const existing = mailboxMessagesByRunId.get(runId);
    if (existing) {
      return existing;
    }
    const initial: TeamActorMessageRecord[] = [];
    mailboxMessagesByRunId.set(runId, initial);
    return initial;
  };

  const buildSyntheticRun = (runId: string): TeamRunRecord => {
    const teamId = inferTeamIdFromRunId(runId);
    const status = inferRunStatusFromRunId(runId);
    return {
      id: runId,
      team_id: teamId,
      context_id: `ctx-${runId}`,
      status,
      input: {},
      created_at: now + 300,
      started_at: status === "submitted" ? null : now + 301,
      ended_at:
        status === "completed" || status === "failed" || status === "canceled"
          ? now + 360
          : null,
    };
  };

  const buildSyntheticSnapshot = (run: TeamRunRecord): TeamRunSnapshotRecord => {
    const team =
      teams.find((item) => item.id === run.team_id) ?? {
        id: run.team_id,
        name: run.team_id,
        description: null,
        spec: {
          leader_member_id: "agent-leader-1",
          members: [{ member_id: "agent-leader-1", role: "leader", model: "codex" }],
          steps: [{ step_key: "leader_plan" }],
        },
        created_at: now,
        updated_at: now,
      };
    const teamMembers = Array.isArray(team.spec.members) ? team.spec.members : [];
    const leaderMemberId = team.spec.leader_member_id ?? teamMembers[0]?.member_id ?? "agent-leader-1";
    const members =
      teamMembers.length > 0
        ? teamMembers.map((member) => {
            const matchedAgent = agents.find((agent) => agent.id === member.member_id);
            const isLeader = member.member_id === leaderMemberId;
            return {
              member_id: member.member_id,
              role: member.role ?? (isLeader ? "leader" : "worker"),
              model: member.model ?? null,
              description: member.description ?? null,
              prompt: null,
              skills: [],
              pending_inbox_count: 0,
              status: isLeader ? run.status : "submitted",
              latest_step: null,
              session_status: matchedAgent?.status ?? "idle",
            };
          })
        : [
            {
              member_id: leaderMemberId,
              role: "leader",
              model: "codex",
              description: null,
              prompt: null,
              skills: [],
              pending_inbox_count: 0,
              status: run.status,
              latest_step: null,
              session_status: "idle",
            },
          ];
    const recentMessages = ensureMailboxMessages(run.id).slice(-20);
    return {
      run,
      team,
      leader_member_id: leaderMemberId,
      members,
      steps: [],
      latest_events: [],
      mailbox: {
        pending: 0,
        delivered: 0,
        dead_letter: 0,
        recent_messages: recentMessages,
      },
    };
  };

  const buildTeamRuntime = (team: TeamDefinitionRecord) => {
    const override = teamRuntimeStateById.get(team.id);
    const members = team.spec.members.map((member) => {
      const agent = agents.find((item) => item.id === member.member_id);
      const agentStatus = override
        ? override.status === "running"
          ? "running"
          : "stopped"
        : agent?.status ?? "created";
      const isRunning = agentStatus === "running";
      const sessionId = isRunning ? `session-${team.id}-${member.member_id}` : null;
      return {
        member_id: member.member_id,
        display_name: agent?.name ?? member.member_id,
        role: member.role ?? "worker",
        description: member.description ?? null,
        agent_status: agentStatus,
        session_id: sessionId,
        session_status: isRunning ? "running" : agentStatus === "stopped" ? "stopped" : null,
        card: {
          card_id: `agenthub://agents/${member.member_id}`,
          schema_version: "agenthub.a2a.discovery_card.v1",
          description: member.description ?? `${member.member_id} runtime`,
          capability_tags: ["team_mailbox_v1"],
        },
      };
    });
    const onlineCount = members.filter((member) => member.session_id).length;
    const status: TeamRuntimeStatus = override?.status ??
      (onlineCount === 0
        ? "stopped"
        : onlineCount === members.length
          ? "running"
          : "degraded");
    return {
      team_id: team.id,
      team_name: team.name,
      status,
      members,
    };
  };

  await page.addInitScript((storedAuth: StoredAuthState) => {
    window.localStorage.setItem("agenthub_auth", JSON.stringify(storedAuth));
  }, auth);

  await page.route("**/api/auth/status", async (route) => {
    await route.fulfill(jsonResponse({ root_initialized: true }));
  });

  await page.route("**/api/agents", async (route, request) => {
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        name: string;
        workdir: string;
        command: string;
        args?: string[];
        code_mode?: boolean;
      };
      const created: E2eAgentRecord = {
        id: `agent-forge-${agents.length + 1}`,
        name: payload.name,
        workdir: payload.workdir,
        command: payload.command,
        args: payload.args ?? [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: payload.code_mode ?? true,
        status: "idle",
        created_at: now,
        updated_at: now,
      };
      agents.push(created);
      await route.fulfill(jsonResponse(created));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(agents));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/agents\/[^/]+\/\.well-known\/agent-card$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const segments = url.pathname.split("/");
    const agentId = segments[segments.length - 3] ?? "";
    const agent = agents.find((item) => item.id === agentId);
    if (!agent) {
      await route.fulfill(jsonResponse({ error: "agent not found" }, 404));
      return;
    }
    const commandName = agent.command.split("/").pop() ?? agent.command;
    const acpProvider =
      commandName === "gemini"
        ? "gemini"
        : commandName === "kimi"
          ? "kimi"
          : "codex";
    const capabilityTags = ["team_mailbox_v1", "team_step_execution_v1"];
    if (agent.code_mode) capabilityTags.push("code_mode");
    if (
      agent.worktree_mode === "create_worktree" ||
      agent.worktree_mode === "reuse_worktree"
    ) {
      capabilityTags.push("git_worktree");
    }
    capabilityTags.push(`acp_${acpProvider}`);
    await route.fulfill(
      jsonResponse({
        card_id: `agenthub://agents/${agent.id}`,
        schema_version: "agenthub.a2a.discovery_card.v1",
        description: `AgentHub team member ${agent.name} (provider: ${acpProvider}) supports ${capabilityTags.join(", ")}`,
        identity: {
          agent_id: agent.id,
          name: agent.name,
          status: agent.status,
        },
        runtime: {
          acp_provider: acpProvider,
          code_mode: agent.code_mode,
          worktree_mode: agent.worktree_mode,
          worktree_repo: agent.worktree_repo ?? null,
          worktree_ref: agent.worktree_ref ?? null,
        },
        capability_tags: capabilityTags,
      })
    );
  });

  await page.route("**/api/teams", async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(teams));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as CreateTeamPayload;
      createTeamPayload = payload;
      const created: TeamDefinitionRecord = {
        id: `team-e2e-${teams.length + 1}`,
        name: payload.name,
        description: payload.description ?? null,
        spec: payload.spec,
        created_at: now,
        updated_at: now,
      };
      teams.push(created);
      teamRuntimeStateById.set(created.id, {
        status: payload.spec.members.length > 0 ? "running" : "stopped",
      });
      await route.fulfill(jsonResponse(created));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.split("/").pop() ?? "";
    if (request.method() === "GET") {
      const found = teams.find((team) => team.id === teamId);
      if (!found) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      await route.fulfill(jsonResponse(found));
      return;
    }
    if (request.method() === "DELETE") {
      const index = teams.findIndex((team) => team.id === teamId);
      if (index < 0) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      const [deleted] = teams.splice(index, 1);
      teamRuntimeStateById.delete(teamId);
      await route.fulfill(jsonResponse(deleted));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/spec$/, async (route, request) => {
    if (request.method() !== "PUT") {
      await route.fallback();
      return;
    }
    const teamId = request.url().match(/\/api\/teams\/([^/]+)\/spec$/)?.[1] ?? "";
    const index = teams.findIndex((team) => team.id === teamId);
    if (index < 0) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    const payload = request.postDataJSON() as UpdateTeamSpecPayload;
    if (payload.expected_updated_at !== teams[index].updated_at) {
      await route.fulfill(jsonResponse({ error: "team spec changed" }, 409));
      return;
    }
    updateSpecPayloads.push({ teamId, payload });
    const updated: TeamDefinitionRecord = {
      ...teams[index],
      spec: payload.spec,
      updated_at: now + updateSpecPayloads.length,
    };
    teams[index] = updated;
    teamRuntimeStateById.set(teamId, {
      status: payload.spec.members.length > 0 ? "running" : "stopped",
    });
    await route.fulfill(jsonResponse(updated));
  });

  await page.route(/\/api\/teams\/[^/]+\/runtime$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runtime$/)?.[1] ?? "";
    const team = teams.find((item) => item.id === teamId);
    if (!team) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(buildTeamRuntime(team)));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/[^/]+\/(start|stop)$/, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const matched = url.pathname.match(/\/api\/teams\/([^/]+)\/(start|stop)$/);
    const teamId = matched?.[1] ?? "";
    const action = matched?.[2] ?? "";
    const team = teams.find((item) => item.id === teamId);
    if (!team) {
      await route.fulfill(jsonResponse({ error: "team not found" }, 404));
      return;
    }
    teamRuntimeStateById.set(team.id, {
      status: action === "start" ? "running" : "stopped",
    });
    await route.fulfill(
      jsonResponse({
        team_id: team.id,
        status: action === "start" ? "running" : "stopped",
        members: team.spec.members.map((member) => ({
          member_id: member.member_id,
          session_id: `session-${team.id}-${member.member_id}`,
          action: action === "start" ? "started" : "stopped",
        })),
      })
    );
  });

  await page.route(/\/api\/teams\/[^/]+\/tasks(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
    if (!teamId) {
      await route.fulfill(jsonResponse({ error: "team id missing" }, 400));
      return;
    }
    if (request.method() === "GET") {
      const tasks = ensureTasks(teamId);
      await route.fulfill(jsonResponse(tasks));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        title?: string;
        created_by_actor_id?: string;
        topic?: string;
        conversation_mode?: "to_leader" | "to_member" | "group_chat";
      };
      const current = ensureTasks(teamId);
      const nextIndex = (taskCounterByTeamId.get(teamId) ?? current.length) + 1;
      taskCounterByTeamId.set(teamId, nextIndex);
      const taskId = `task-${teamId}-${nextIndex}`;
      const createdAt = now + 100 + nextIndex;
      const createdTask: TeamTaskRecord = {
        id: taskId,
        team_id: teamId,
        title: payload.title?.trim() || `Conversation ${nextIndex}`,
        status: "open",
        created_by_actor_id: payload.created_by_actor_id ?? `user:${auth.userId}`,
        context: {
          topic: payload.topic ?? null,
          conversation_mode: payload.conversation_mode ?? "to_leader",
        },
        created_at: createdAt,
        updated_at: createdAt,
      };
      tasksByTeamId.set(teamId, [createdTask, ...current.filter((item) => item.id !== taskId)]);
      taskMessagesById.set(taskId, []);
      await route.fulfill(jsonResponse({ task: createdTask }));
      return;
    }
    await route.fallback();
  });

  await page.route(
    /\/api\/teams\/[^/]+\/tasks\/[^/]+\/messages(?:\?.*)?$/,
    async (route, request) => {
      const url = new URL(request.url());
      const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
      const taskId =
        url.pathname.match(/\/tasks\/([^/]+)\/messages(?:\?.*)?$/)?.[1] ?? "";
      if (!teamId || !taskId) {
        await route.fulfill(jsonResponse({ error: "path params missing" }, 400));
        return;
      }
      const tasks = ensureTasks(teamId);
      const task = tasks.find((item) => item.id === taskId);
      if (!task) {
        await route.fulfill(jsonResponse({ error: "task not found" }, 404));
        return;
      }
      const existingMessages = taskMessagesById.get(taskId) ?? [];
      if (request.method() === "GET") {
        await route.fulfill(jsonResponse(existingMessages));
        return;
      }
      if (request.method() === "POST") {
        const payload = request.postDataJSON() as {
          from_actor_id?: string;
          to_actor_id?: string | null;
          route?: "to_leader" | "to_member" | "group_chat";
          payload?: unknown;
        };
        const nextMessageId =
          (existingMessages.length > 0
            ? existingMessages[existingMessages.length - 1]?.message_id ?? 0
            : 0) + 1;
        const createdMessage: TeamConversationMessageRecord = {
          message_id: nextMessageId,
          conversation_id: `conversation-${taskId}`,
          task_id: taskId,
          from_actor_id: payload.from_actor_id ?? `user:${auth.userId}`,
          to_actor_id:
            payload.to_actor_id ??
            (payload.route === "to_member" ? "agent-worker-1" : "agent-leader-1"),
          route: payload.route ?? "to_leader",
          payload: payload.payload ?? { type: "chat_message", text: "" },
          created_at: now + 200 + nextMessageId,
        };
        const nextMessages = [...existingMessages, createdMessage];
        taskMessagesById.set(taskId, nextMessages);
        await route.fulfill(jsonResponse(createdMessage));
        return;
      }
      await route.fallback();
    }
  );

  await page.route(
    /\/api\/teams\/[^/]+\/tasks\/[^/]+\/compile_run_preview$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const url = new URL(request.url());
      const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/tasks/)?.[1] ?? "";
      const taskId =
        url.pathname.match(/\/tasks\/([^/]+)\/compile_run_preview$/)?.[1] ?? "";
      if (!teamId || !taskId) {
        await route.fulfill(jsonResponse({ error: "path params missing" }, 400));
        return;
      }
      const task = ensureTasks(teamId).find((item) => item.id === taskId);
      if (!task) {
        await route.fulfill(jsonResponse({ error: "task not found" }, 404));
        return;
      }
      const team = teams.find((item) => item.id === teamId);
      const leaderMemberId = team?.spec.leader_member_id ?? "agent-leader-1";
      const workerMemberId =
        team?.spec.members.find((member) => member.role === "worker")?.member_id ??
        leaderMemberId;
      const messageList = taskMessagesById.get(taskId) ?? [];
      const latestMessageId = messageList.length > 0 ? messageList[messageList.length - 1]?.message_id ?? 0 : 0;
      await route.fulfill(
        jsonResponse({
          task_id: taskId,
          conversation_id: `conversation-${taskId}`,
          run_payload: {
            context_id: `ctx-${taskId}`,
            input: {
              task_compile_version: 1,
              task_id: taskId,
              conversation_id: `conversation-${taskId}`,
              tool_name: "tiny-json-cli",
              objective: task.title,
              task_list: [
                "Define tiny CLI interface",
                "Implement parse/format commands",
                "Add tests and usage docs",
              ],
            },
          },
          plan: {
            task_list: [
              "Define tiny CLI interface",
              "Implement parse/format commands",
              "Add tests and usage docs",
            ],
            acceptance_criteria: [
              "CLI can parse and pretty-print JSON",
              "Unit tests pass for main happy paths",
            ],
            deadline: "2026-03-20",
            step_template: [
              {
                step_key: "leader_plan",
                member_id: leaderMemberId,
                role: "leader",
                depends_on: [],
              },
              {
                step_key: "worker_build_tool",
                member_id: workerMemberId,
                role: "worker",
                depends_on: ["leader_plan"],
              },
              {
                step_key: "leader_synthesize",
                member_id: leaderMemberId,
                role: "leader",
                depends_on: ["worker_build_tool"],
              },
            ],
            role_assignments: [
              {
                member_id: leaderMemberId,
                role: "leader",
                step_keys: ["leader_plan", "leader_synthesize"],
              },
              {
                member_id: workerMemberId,
                role: "worker",
                step_keys: ["worker_build_tool"],
              },
            ],
            source_message_id: latestMessageId,
          },
        })
      );
    }
  );

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().split("/").pop() ?? "";
    await route.fulfill(jsonResponse(buildSyntheticRun(runId)));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/steps$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/snapshot(?:\?.*)?$/,
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const runId =
        request.url().match(/\/api\/teams\/runs\/([^/]+)\/snapshot/)?.[1] ?? "";
      const run = buildSyntheticRun(runId);
      await route.fulfill(jsonResponse(buildSyntheticSnapshot(run)));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/inbox(?:\?.*)?$/,
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/inbox/)?.[1] ?? "";
      await route.fulfill(jsonResponse(ensureMailboxMessages(runId)));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/send$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/send/)?.[1] ?? "";
      const payload = request.postDataJSON() as {
        from_actor_id?: string;
        to_actor_id?: string;
        channel?: string;
        transport?: "local" | "remote";
        route?: Record<string, unknown> | null;
        payload?: unknown;
      };
      const messages = ensureMailboxMessages(runId);
      const nextMessageId =
        (messages[messages.length - 1]?.message_id ?? runEventCounterByRunId.get(runId) ?? 0) + 1;
      runEventCounterByRunId.set(runId, nextMessageId);
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id ?? `user:${auth.userId}`,
        from_actor_kind:
          (payload.from_actor_id ?? "").startsWith("user:") ? "human" : "agent",
        to_actor_id: payload.to_actor_id ?? "agent-leader-1",
        to_actor_kind:
          (payload.to_actor_id ?? "agent-leader-1").startsWith("user:")
            ? "human"
            : "agent",
        channel: payload.channel ?? "default",
        transport: payload.transport ?? "local",
        route: payload.route ?? null,
        payload: payload.payload ?? {},
        status: "pending",
        created_at: now + nextMessageId,
        delivered_at: null,
      };
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/messages\/\d+\/ack$/,
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/messages\/\d+\/ack/)?.[1] ?? "";
      const messageId = Number(request.url().match(/\/messages\/(\d+)\/ack$/)?.[1] ?? 0);
      const messages = ensureMailboxMessages(runId);
      const found = messages.find((message) => message.message_id === messageId);
      if (!found) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      const delivered: TeamActorMessageRecord = {
        ...found,
        status: "delivered",
        delivered_at: now + messageId + 1,
      };
      const next = messages.map((message) =>
        message.message_id === messageId ? delivered : message
      );
      mailboxMessagesByRunId.set(runId, next);
      await route.fulfill(jsonResponse(delivered));
    }
  );

  return {
    now,
    auth,
    agents,
    teams,
    getCreatePayload: () => createTeamPayload,
    getUpdateSpecPayloads: () => updateSpecPayloads,
  };
}

test("team runtime controls update shared runtime badge", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push({
    id: "team-runtime-controls",
    name: "runtime controls team",
    description: "runtime badge coverage",
    spec: {
      leader_member_id: "agent-leader-1",
      members: [
        { member_id: "agent-leader-1", role: "leader", description: "lead" },
        { member_id: "agent-worker-1", role: "worker", description: "worker" },
      ],
      steps: [{ step_key: "leader_plan" }, { step_key: "worker_exec" }],
    },
    created_at: fixture.now + 20,
    updated_at: fixture.now + 20,
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "runtime controls team");
  await expectTeamRuntimeBadge(page, "team running");

  await clickSelectedTeamMenuItem(page, "Stop Team");
  await expectTeamRuntimeBadge(page, "team stopped");

  await clickSelectedTeamMenuItem(page, "Start Team");
  await expectTeamRuntimeBadge(page, "team running");
});

test("team create flow stores mission metadata before member setup", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);

  await gotoTeams(page);

  await expect(page.getByRole("heading", { name: "Team Selector" })).toBeVisible();
  await createTeamFromModal(page, {
    name: "quest-team",
    goal: "Build a goal-first team and add members afterward.",
  });
  await expect(page).toHaveURL(/\/teams\/.+/);
  await expect.poll(() => isTeamDetailReady(page)).toBe(true);
  await openSelectedTeamMenu(page);
  await expect(page.getByRole("menuitem", { name: "Add Agent", exact: true })).toBeVisible();
  await expect(page.getByText("No agents have joined this team yet.")).toBeVisible();

  const payload = fixture.getCreatePayload();
  expect(payload).not.toBeNull();
  const createdPayload = payload as CreateTeamPayload;
  expect(createdPayload.name).toBe("quest-team");
  expect(createdPayload.description).toBe("Build a goal-first team and add members afterward.");
  expect(createdPayload.spec).toEqual({
    spec_version: 1,
    members: [],
  });
});

test("team member setup adds the first agent and appends more agents through spec updates", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamName = "member-setup-team";

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: teamName,
    goal: "Create team first, then configure leader and worker profiles.",
  });

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/member-setup-leader",
    model: "codex",
    identity: "Principal planner and reviewer",
  });
  await openTeamFromSelector(page, teamName);
  await expectAddAgentEntryVisible(page, teamName);

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/member-setup-worker",
    model: "gemini",
    identity: "Implementation specialist",
  });

  const updates = fixture.getUpdateSpecPayloads();
  expect(updates).toHaveLength(2);
  expect(updates[0]?.payload.spec.leader_member_id).toBe("agent-forge-4");
  expect(updates[0]?.payload.spec.members.map((member) => member.role)).toEqual(["leader"]);
  expect(updates[1]?.payload.spec.members.map((member) => member.role)).toEqual([
    "leader",
    "worker",
  ]);
  const [leaderMember, workerMember] = updates[1]?.payload.spec.members ?? [];
  expect(leaderMember?.model).toBe("codex");
  expect(leaderMember?.skills).toBeUndefined();
  expect(workerMember?.model).toBe("gemini");
  expect(workerMember?.skills).toBeUndefined();
  expect(updates[1]?.payload.spec.steps?.map((step) => step.step_key)).toEqual([
    "leader_plan",
    "worker_1_agent_forge_5",
    "leader_synthesize",
  ]);
});

test("team create modal only captures mission metadata and points member setup to the next step", async ({
  page,
}) => {
  await mockTeamPageApis(page);
  await gotoTeams(page);

  await openMainTeamAction(page, "Create Team");
  const dialog = page
    .locator("[role='dialog']")
    .filter({ hasText: "Start with the mission" })
    .last();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Team name")).toBeVisible();
  await expect(dialog.getByLabel("Team goal")).toBeVisible();
  await expect(dialog.getByRole("button", { name: /Add Agent|New Agent/ })).toHaveCount(0);
  await expect(dialog.getByText("Add agents afterward")).toBeVisible();
});

test("team page keeps single-column proportions on mobile viewport", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longLeaderId = `agent-leader-${"x".repeat(72)}`;
  const longWorkerId = `agent-worker-${"y".repeat(72)}`;
  fixture.teams.push({
    id: "team-mobile",
    name: "Team Mobile",
    description: "mobile layout regression guard",
    spec: {
      leader_member_id: longLeaderId,
      members: [
        { member_id: longLeaderId, role: "leader", model: "codex" },
        { member_id: longWorkerId, role: "worker", model: "gemini" },
      ],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });

  await page.setViewportSize({ width: 390, height: 844 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Mobile");
  await openMainTeamAction(page, "Runs");

  await expect(page.locator(".teams-main").getByText("Team Mobile", { exact: true })).toBeVisible();

  const layoutColumns = await page.locator(".teams-layout").evaluate((element) => {
    return window.getComputedStyle(element).gridTemplateColumns;
  });
  expect(layoutColumns.trim().split(/\s+/).length).toBe(1);

  const { runFilterWidth, runFilterParentWidth } = await page
    .locator(".teams-run-list-head .actions")
    .first()
    .evaluate((element) => {
      const select = element.querySelector("select");
      return {
        runFilterWidth: select?.getBoundingClientRect().width ?? 0,
        runFilterParentWidth: element.getBoundingClientRect().width,
      };
    });
  expect(runFilterWidth).toBeGreaterThan(runFilterParentWidth * 0.7);

  const horizontalOverflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth - document.documentElement.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
});

test("team page desktop keeps long metadata blocks non-overlapping", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const longLeaderId = `leader-${"l".repeat(96)}`;
  const longWorkerId1 = `worker-a-${"w".repeat(88)}`;
  const longWorkerId2 = `worker-b-${"z".repeat(88)}`;
  const longPrompt = `prompt-${"p".repeat(420)}`;
  const teamId = "team-desktop";
  fixture.teams.push({
    id: teamId,
    name: "Team Desktop",
    description: "desktop overlap regression guard",
    spec: {
      leader_member_id: longLeaderId,
      members: [
        {
          member_id: longLeaderId,
          role: "leader",
          model: "codex",
          skills: ["agenthub-actor-runtime", "team-leader-orchestrator", `mcp-${"m".repeat(52)}`],
        },
        {
          member_id: longWorkerId1,
          role: "worker",
          model: "gemini",
          skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"a".repeat(52)}`],
        },
        {
          member_id: longWorkerId2,
          role: "worker",
          model: "kimi",
          skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"b".repeat(52)}`],
        },
      ],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });
  const runRecord = buildTeamRun(teamId, "working", fixture.now + 10, 1);
  const runId = runRecord.id;
  const runEvents: Array<Record<string, unknown>> = [];

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

  await page.route(new RegExp(`/api/teams/runs/${runId}/steps$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/events(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(runEvents));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      await route.fulfill(
        jsonResponse({
          run: runRecord,
          team: fixture.teams.find((team) => team.id === teamId),
          leader_member_id: longLeaderId,
          members: [
            {
              member_id: longLeaderId,
              role: "leader",
              model: "codex",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-leader-orchestrator", `mcp-${"m".repeat(52)}`],
              pending_inbox_count: 0,
              status: "working",
              latest_step: {
                id: "step-leader",
                run_id: runId,
                step_key: "leader_plan",
                member_id: longLeaderId,
                remote_task_id: `remote-${"r".repeat(64)}`,
                status: "working",
                attempt: 1,
                depends_on: [],
                input: {},
                output: null,
                error_text: null,
                started_at: fixture.now + 11,
                ended_at: null,
              },
              session_status: "working",
            },
            {
              member_id: longWorkerId1,
              role: "worker",
              model: "gemini",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"a".repeat(52)}`],
              pending_inbox_count: 0,
              status: "submitted",
              latest_step: null,
              session_status: "idle",
            },
            {
              member_id: longWorkerId2,
              role: "worker",
              model: "kimi",
              prompt: longPrompt,
              skills: ["agenthub-actor-runtime", "team-worker-executor", `mcp-${"b".repeat(52)}`],
              pending_inbox_count: 0,
              status: "submitted",
              latest_step: null,
              session_status: "idle",
            },
          ],
          steps: [],
          latest_events: [],
          mailbox: {
            pending: 0,
            delivered: 0,
            dead_letter: 0,
            recent_messages: [],
          },
        })
      );
    }
  );

  await page.route(new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.setViewportSize({ width: 1366, height: 900 });
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Desktop");
  await openMainTeamAction(page, "Runs");
  await expect(page.locator(".teams-main").getByText("Team Desktop", { exact: true })).toBeVisible();
  await openAdvancedView(page, "Overview");
  await expect(page.locator(".teams-member-list .team-member-row")).toHaveCount(3);
  await expect(page.locator(".teams-overview-meta")).toBeVisible();

  const overviewLayout = await page.evaluate(() => {
    const selectors = [".teams-overview-meta", ".teams-member-list"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(overviewLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(overviewLayout.overflowing).toEqual([]);

  await selectAgentFromSidebar(page, longLeaderId);
  await openAdvancedView(page, "Member Console");
  const memberConsoleCard = page.locator(".card", { hasText: "Member Console" });
  await expect(memberConsoleCard).toBeVisible();
  await memberConsoleCard.locator("select").first().selectOption(longLeaderId);
  await expect(memberConsoleCard).toContainText("mcp_skills");
  await memberConsoleCard.locator("summary", { hasText: "prompt" }).click();

  const memberConsoleLayout = await page.evaluate(() => {
    const selectors = [".teams-step-body"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(memberConsoleLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(memberConsoleLayout.overflowing).toEqual([]);

  await selectPrimaryTeamEntryFromSidebar(page, "all");
  await expect(page.getByRole("heading", { name: "# all", exact: true })).toBeVisible();
  await openAdvancedView(page, "Execution Mailbox");
  await expect(page.locator(".teams-chat-head")).toBeVisible();
  const mailboxLayout = await page.evaluate(() => {
    const selectors = [".teams-chat-head"];
    const overflowing = selectors.filter((selector) => {
      const node = document.querySelector(selector) as HTMLElement | null;
      if (!node) return false;
      return node.scrollWidth - node.clientWidth > 1;
    });
    return {
      docOverflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      overflowing,
    };
  });
  expect(mailboxLayout.docOverflow).toBeLessThanOrEqual(1);
  expect(mailboxLayout.overflowing).toEqual([]);
});

test("team setup keeps add agent wording after the first member binds", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  fixture.agents.splice(0, fixture.agents.length);
  const teamName = "forge-team";

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: teamName,
    goal: "Bind leader in-place before worker setup.",
  });
  await expectAddAgentEntryVisible(page, teamName);
  await expect(page.getByText("No agents have joined this team yet.")).toBeVisible();

  await createTeamMemberFromModal(page, {
    teamName,
    workdir: "/workspace/forge-leader",
    identity: "Leader bound in-place",
  });

  await openTeamFromSelector(page, teamName);
  await expectAddAgentEntryVisible(page, teamName);
  const updates = fixture.getUpdateSpecPayloads();
  expect(updates).toHaveLength(1);
  expect(updates[0]?.payload.spec.members[0]?.member_id).toBe("agent-forge-1");
});

test("team quant workflow creates team and launches run", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const runsByTeamId = new Map<string, TeamRunRecord[]>();
  const runById = new Map<string, TeamRunRecord>();
  const runStepsById = new Map<string, Array<Record<string, unknown>>>();
  const runEventsById = new Map<string, Array<Record<string, unknown>>>();
  const nextRunIndexByTeamId = new Map<string, number>();

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/)?.[1] ?? "";
    if (!teamId) {
      await route.fallback();
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as {
        context_id?: string;
        input?: Record<string, unknown>;
      };
      const nextIndex = (nextRunIndexByTeamId.get(teamId) ?? 0) + 1;
      nextRunIndexByTeamId.set(teamId, nextIndex);
      const createdAt = fixture.now + 1_000 + nextIndex;
      const run: TeamRunRecord = {
        id: `${teamId}-quant-run-${nextIndex}`,
        team_id: teamId,
        context_id: payload.context_id ?? `ctx-${teamId}-${nextIndex}`,
        status: "working",
        input: payload.input ?? {},
        created_at: createdAt,
        started_at: createdAt + 1,
        ended_at: null,
      };
      const prev = runsByTeamId.get(teamId) ?? [];
      runsByTeamId.set(teamId, [run, ...prev]);
      runById.set(run.id, run);
      runStepsById.set(run.id, [
        {
          id: `${run.id}-step-1`,
          run_id: run.id,
          step_key: "leader_plan",
          member_id: "quant-leader",
          remote_task_id: "task-leader-plan",
          status: "working",
          attempt: 1,
          depends_on: [],
          input: run.input,
          output: null,
          error_text: null,
          started_at: createdAt + 2,
          ended_at: null,
        },
        {
          id: `${run.id}-step-2`,
          run_id: run.id,
          step_key: "worker_portfolio_optimize",
          member_id: "portfolio-worker",
          remote_task_id: null,
          status: "submitted",
          attempt: 1,
          depends_on: ["leader_plan"],
          input: {},
          output: null,
          error_text: null,
          started_at: null,
          ended_at: null,
        },
        {
          id: `${run.id}-step-3`,
          run_id: run.id,
          step_key: "worker_crypto_algo_trade",
          member_id: "crypto-worker",
          remote_task_id: null,
          status: "submitted",
          attempt: 1,
          depends_on: ["leader_plan"],
          input: {},
          output: null,
          error_text: null,
          started_at: null,
          ended_at: null,
        },
      ]);
      runEventsById.set(run.id, [
        {
          event_id: 1,
          run_id: run.id,
          step_id: null,
          event_type: "run_submitted",
          ts: createdAt,
          payload: { status: "submitted" },
        },
        {
          event_id: 2,
          run_id: run.id,
          step_id: `${run.id}-step-1`,
          event_type: "run_working",
          ts: createdAt + 1,
          payload: { status: "working" },
        },
      ]);
      await route.fulfill(jsonResponse(run));
      return;
    }
    if (request.method() === "GET") {
      const status = url.searchParams.get("status");
      const beforeCreatedAtRaw = url.searchParams.get("before_created_at");
      const beforeCreatedAt =
        beforeCreatedAtRaw == null ? null : Number(beforeCreatedAtRaw);
      const limitRaw = Number(url.searchParams.get("limit") ?? "50");
      const limit = Number.isFinite(limitRaw) && limitRaw > 0 ? limitRaw : 50;
      const base = runsByTeamId.get(teamId) ?? [];
      const filtered = base
        .filter((run) => (status && status !== "all" ? run.status === status : true))
        .filter((run) =>
          beforeCreatedAt == null ? true : run.created_at < beforeCreatedAt
        );
      await route.fulfill(jsonResponse(filtered.slice(0, limit)));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/?]+)/)?.[1] ?? "";
    const run = runById.get(runId);
    if (!run) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(run));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/steps$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/steps$/)?.[1] ?? "";
    await route.fulfill(jsonResponse(runStepsById.get(runId) ?? []));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const runId = request.url().match(/\/api\/teams\/runs\/([^/]+)\/events/ )?.[1] ?? "";
    await route.fulfill(jsonResponse(runEventsById.get(runId) ?? []));
  });

  await page.route(
    /\/api\/teams\/runs\/[^/]+\/snapshot(?:\?.*)?$/,
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const runId =
        request.url().match(/\/api\/teams\/runs\/([^/]+)\/snapshot/)?.[1] ?? "";
      const run = runById.get(runId);
      if (!run) {
        await route.fulfill(jsonResponse({ error: "run not found" }, 404));
        return;
      }
      const team = fixture.teams.find((item) => item.id === run.team_id);
      if (!team) {
        await route.fulfill(jsonResponse({ error: "team not found" }, 404));
        return;
      }
      const members = (team.spec.members ?? []).map((member, index) => ({
        member_id: member.member_id,
        role: member.role ?? "worker",
        model: member.model ?? null,
        prompt: null,
        skills: member.skills ?? [],
        pending_inbox_count: 0,
        status: index === 0 ? "working" : "submitted",
        latest_step: null,
        session_status: index === 0 ? "working" : "idle",
      }));
      await route.fulfill(
        jsonResponse({
          run,
          team,
          leader_member_id: team.spec.leader_member_id,
          members,
          steps: runStepsById.get(run.id) ?? [],
          latest_events: runEventsById.get(run.id) ?? [],
          mailbox: {
            pending: 0,
            delivered: 0,
            dead_letter: 0,
            recent_messages: [],
          },
        })
      );
    }
  );

  const quantSpec = {
    spec_version: 1,
    entrypoint: "leader_plan",
    leader_member_id: "quant-leader",
    members: [
      {
        member_id: "quant-leader",
        role: "leader",
        model: "codex",
        prompt: "Own run-level planning, risk budget, and compute/resource control.",
        skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
      },
      {
        member_id: "portfolio-worker",
        role: "worker",
        model: "gemini",
        prompt: "Do portfolio optimization with risk-parity and exposure constraints.",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
      },
      {
        member_id: "crypto-worker",
        role: "worker",
        model: "kimi",
        prompt: "Run crypto algo trading simulation and report pnl/drawdown.",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
      },
    ],
    steps: [
      { step_key: "leader_plan", member_id: "quant-leader", depends_on: [] },
      {
        step_key: "worker_portfolio_optimize",
        member_id: "portfolio-worker",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "worker_crypto_algo_trade",
        member_id: "crypto-worker",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "leader_synthesize",
        member_id: "quant-leader",
        depends_on: ["worker_portfolio_optimize", "worker_crypto_algo_trade"],
      },
    ],
  };

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: "quant-alpha-desk",
    goal: "leader manages resources; workers optimize portfolio + crypto trading",
  });
  fixture.agents.push(
    {
      id: "quant-leader",
      name: "quant-leader",
      workdir: "/workspace/quant-leader",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 200,
      updated_at: fixture.now + 200,
    },
    {
      id: "portfolio-worker",
      name: "portfolio-worker",
      workdir: "/workspace/portfolio-worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 201,
      updated_at: fixture.now + 201,
    },
    {
      id: "crypto-worker",
      name: "crypto-worker",
      workdir: "/workspace/crypto-worker",
      command: "kimi",
      args: ["acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 202,
      updated_at: fixture.now + 202,
    }
  );
  const createdTeam = fixture.teams.find((team) => team.name === "quant-alpha-desk");
  if (!createdTeam) {
    throw new Error("quant-alpha-desk was not created");
  }
  createdTeam.spec = quantSpec;
  createdTeam.updated_at += 1;
  await gotoTeams(page);
  await openTeamFromSelector(page, "quant-alpha-desk");

  const createPayload = fixture.getCreatePayload();
  expect(createPayload).not.toBeNull();
  expect((createPayload as CreateTeamPayload).spec).toEqual({
    spec_version: 1,
    members: [],
  });

  await openAdvancedView(page, "Debug");
  await page
    .getByPlaceholder("context_id (optional, auto-generated when empty)")
    .fill("quant-run-ctx");
  await page
    .getByLabel("Run input JSON")
    .fill('{"objective":"daily rebalance + crypto hedge","risk_limit":"max_dd_5pct"}');
  await page.getByRole("button", { name: "Create Run", exact: true }).click();

  await openMainTeamAction(page, "Runs");
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "quant-run-1"
  );
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(
    "working"
  );
  await openAdvancedView(page, "Overview");
  await expect(page.locator(".team-member-row", { hasText: "quant-leader" })).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "portfolio-worker" })
  ).toBeVisible();
  await expect(
    page.locator(".team-member-row", { hasText: "crypto-worker" })
  ).toBeVisible();
});

test("team debug run ops compiles task preview and applies payload to create-run form", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const teamId = "team-compile";
  const teamCreatedAt = fixture.now + 120;
  const previewResponse = {
    task_id: "task-compile-1",
    conversation_id: "conversation-compile-1",
    run_payload: {
      context_id: "ctx-task-compile-1",
      input: {
        task_compile_version: 1,
        task_id: "task-compile-1",
        task_list: ["Implement compile preview", "Wire run ops"],
      },
    },
    plan: {
      task_list: ["Implement compile preview", "Wire run ops"],
      acceptance_criteria: ["Compile payload is deterministic"],
      deadline: "2026-03-08",
      step_template: [
        {
          step_key: "leader_plan",
          member_id: "planner",
          role: "leader",
          depends_on: [],
        },
      ],
      role_assignments: [
        {
          member_id: "planner",
          role: "leader",
          step_keys: ["leader_plan"],
        },
      ],
      source_message_id: 12,
    },
  };
  const compileRequests: Array<{ context_id?: string }> = [];

  fixture.teams.push({
    id: teamId,
    name: "Compile Team",
    description: "compile preview e2e",
    spec: {
      leader_member_id: "planner",
      members: [{ member_id: "planner", role: "leader", model: "codex" }],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  await page.route(
    new RegExp(`/api/teams/${teamId}/tasks/[^/]+/compile_run_preview$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const payload = request.postDataJSON() as { context_id?: string };
      compileRequests.push(payload);
      await route.fulfill(jsonResponse(previewResponse));
    }
  );

  await gotoTeams(page);
  await openTeamFromSelector(page, "Compile Team");
  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);

  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();

  await expect(page.getByText("conversation-compile-1", { exact: true })).toBeVisible();
  await expect(page.getByText("ctx-task-compile-1", { exact: true })).toBeVisible();
  expect(compileRequests).toEqual([{}]);

  await page.getByRole("button", { name: "Use Payload in Create Run" }).click();
  await openAdvancedView(page, "Debug");
  await expect(
    page.getByPlaceholder("context_id (optional, auto-generated when empty)")
  ).toHaveValue("ctx-task-compile-1");
  await expect(page.getByLabel("Run input JSON")).toContainText(
    '"task_id": "task-compile-1"'
  );
});

test("team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  await enableDeveloperMode(page);
  const teamId = "team-chat-first";
  const runId = "run-chat-first-1";
  const teamCreatedAt = fixture.now + 180;
  const runCreatedAt = fixture.now + 260;
  const previewResponse = {
    task_id: "task-chat-1",
    conversation_id: "conversation-chat-1",
    run_payload: {
      context_id: "ctx-chat-first-1",
      input: {
        task_compile_version: 1,
        task_id: "task-chat-1",
        conversation_id: "conversation-chat-1",
        task_list: [
          "Negotiate scope with leader",
          "Worker implements endpoint",
          "Leader synthesizes final deliverable",
        ],
      },
    },
    plan: {
      task_list: [
        "Negotiate scope with leader",
        "Worker implements endpoint",
        "Leader synthesizes final deliverable",
      ],
      acceptance_criteria: ["Endpoint implemented", "Final summary delivered"],
      deadline: "2026-03-12",
      step_template: [
        {
          step_key: "leader_plan",
          member_id: "agent-leader-1",
          role: "leader",
          depends_on: [],
        },
        {
          step_key: "worker_execute",
          member_id: "agent-worker-1",
          role: "worker",
          depends_on: ["leader_plan"],
        },
        {
          step_key: "leader_synthesize",
          member_id: "agent-leader-1",
          role: "leader",
          depends_on: ["worker_execute"],
        },
      ],
      role_assignments: [
        {
          member_id: "agent-leader-1",
          role: "leader",
          step_keys: ["leader_plan", "leader_synthesize"],
        },
        {
          member_id: "agent-worker-1",
          role: "worker",
          step_keys: ["worker_execute"],
        },
      ],
      source_message_id: 18,
    },
  };

  fixture.teams.push({
    id: teamId,
    name: "Chat First Team",
    description: "chat-first e2e flow",
    spec: {
      leader_member_id: "agent-leader-1",
      members: [
        { member_id: "agent-leader-1", role: "leader", model: "codex" },
        { member_id: "agent-worker-1", role: "worker", model: "gemini" },
      ],
      steps: [
        { step_key: "leader_plan" },
        { step_key: "worker_execute" },
        { step_key: "leader_synthesize" },
      ],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  let activeRun: TeamRunRecord | null = null;
  let nextMessageId = 50;
  const createRunRequests: Array<{ context_id?: string; input?: unknown }> = [];
  const sentAcpInputs: Array<{
    agent_id: string;
    input: string;
    session_id?: string;
  }> = [];
  const messages: TeamActorMessageRecord[] = [
    {
      message_id: 1,
      run_id: runId,
      from_actor_id: "agent-leader-1",
      to_actor_id: "agent-worker-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "chat_message",
        text: "Please implement endpoint scaffolding and tests.",
      },
      status: "pending",
      created_at: runCreatedAt + 1,
      delivered_at: null,
    },
    {
      message_id: 2,
      run_id: runId,
      from_actor_id: "agent-worker-1",
      to_actor_id: "agent-leader-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "worker_status",
        status: "done",
        result: "Endpoint and tests are complete.",
        evidence: ["go test ./..."],
      },
      status: "pending",
      created_at: runCreatedAt + 2,
      delivered_at: null,
    },
  ];

  const runEvents = [
    {
      event_id: 201,
      run_id: runId,
      step_id: "step-worker-execute",
      event_type: "step_completed",
      ts: runCreatedAt + 10,
      payload: {
        step_key: "worker_execute",
        summary: "Worker implementation completed with tests.",
      },
    },
    {
      event_id: 202,
      run_id: runId,
      step_id: "step-leader-synthesize",
      event_type: "leader_synthesized",
      ts: runCreatedAt + 20,
      payload: {
        final_deliverable: "Final deliverable prepared and returned to user.",
      },
    },
  ];

  const buildSnapshot = () => {
    if (!activeRun) {
      return null;
    }
    return {
      run: activeRun,
      team: fixture.teams.find((team) => team.id === teamId),
      leader_member_id: "agent-leader-1",
      members: [
        {
          member_id: "agent-leader-1",
          role: "leader",
          model: "codex",
          prompt: "leader prompt",
          skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
          pending_inbox_count: messages.filter(
            (message) =>
              message.to_actor_id === "agent-leader-1" &&
              message.status === "pending"
          ).length,
          status: "working",
          latest_step: {
            id: "step-leader-synthesize",
            run_id: runId,
            step_key: "leader_synthesize",
            member_id: "agent-leader-1",
            remote_task_id: "task-leader-1",
            status: "working",
            attempt: 1,
            depends_on: ["worker_execute"],
            input: {},
            output: null,
            error_text: null,
            started_at: runCreatedAt + 8,
            ended_at: null,
          },
          session_status: "working",
        },
        {
          member_id: "agent-worker-1",
          role: "worker",
          model: "gemini",
          prompt: "worker prompt",
          skills: ["agenthub-actor-runtime", "team-worker-executor"],
          pending_inbox_count: messages.filter(
            (message) =>
              message.to_actor_id === "agent-worker-1" &&
              message.status === "pending"
          ).length,
          status: "working",
          latest_step: {
            id: "step-worker-execute",
            run_id: runId,
            step_key: "worker_execute",
            member_id: "agent-worker-1",
            remote_task_id: "task-worker-1",
            status: "completed",
            attempt: 1,
            depends_on: ["leader_plan"],
            input: {},
            output: { summary: "done" },
            error_text: null,
            started_at: runCreatedAt + 4,
            ended_at: runCreatedAt + 9,
          },
          session_status: "idle",
        },
      ],
      steps: [],
      latest_events: runEvents,
      mailbox: {
        pending: messages.filter((message) => message.status === "pending").length,
        delivered: messages.filter((message) => message.status === "delivered").length,
        dead_letter: 0,
        recent_messages: [...messages].sort((left, right) => left.message_id - right.message_id),
      },
    };
  };

  await page.route(
    new RegExp(`/api/teams/${teamId}/tasks/[^/]+/compile_run_preview$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      await route.fulfill(jsonResponse(previewResponse));
    }
  );

  await page.route(new RegExp(`/api/teams/${teamId}/runs(?:\\?.*)?$`), async (route, request) => {
    if (request.method() === "GET") {
      await route.fulfill(jsonResponse(activeRun ? [activeRun] : []));
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as { context_id?: string; input?: unknown };
      createRunRequests.push(payload);
      activeRun = {
        id: runId,
        team_id: teamId,
        context_id: payload.context_id ?? "ctx-chat-first-fallback",
        status: "working",
        input: (payload.input as Record<string, unknown>) ?? {},
        created_at: runCreatedAt,
        started_at: runCreatedAt + 1,
        ended_at: null,
      };
      await route.fulfill(jsonResponse(activeRun));
      return;
    }
    await route.fallback();
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(activeRun));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/steps$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/events(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse(activeRun ? runEvents : []));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const snapshot = buildSnapshot();
      if (!snapshot) {
        await route.fulfill(jsonResponse({ error: "snapshot unavailable" }, 404));
        return;
      }
      await route.fulfill(jsonResponse(snapshot));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      const url = new URL(request.url());
      const actorId = url.searchParams.get("actor_id") ?? "";
      const includeDelivered = url.searchParams.get("include_delivered") === "true";
      const inboxMessages = messages
        .filter((message) => message.to_actor_id === actorId)
        .filter((message) => includeDelivered || message.status !== "delivered")
        .sort((left, right) => left.message_id - right.message_id);
      await route.fulfill(jsonResponse(inboxMessages));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/send$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const payload = request.postDataJSON() as {
        from_actor_id: string;
        to_actor_id: string;
        payload: unknown;
      };
      sentMessagePayloads.push({
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        payload: payload.payload,
      });
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        channel: "default",
        transport: "local",
        route: null,
        payload: payload.payload,
        status: "pending",
        created_at: runCreatedAt + nextMessageId,
        delivered_at: null,
      };
      nextMessageId += 1;
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/\\d+/ack$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const messageIdMatch = request.url().match(/messages\/(\d+)\/ack$/);
      const messageId = Number(messageIdMatch?.[1] ?? "0");
      const message = messages.find((item) => item.message_id === messageId);
      if (!message) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      message.status = "delivered";
      message.delivered_at = runCreatedAt + messageId;
      await route.fulfill(jsonResponse(message));
    }
  );

  await page.route(/\/api\/agents\/[^/]+\/input$/, async (route, request) => {
    if (request.method() !== "POST") {
      await route.fallback();
      return;
    }
    const match = request.url().match(/\/api\/agents\/([^/]+)\/input$/);
    const agentId = decodeURIComponent(match?.[1] ?? "");
    const payload = request.postDataJSON() as {
      input: string;
      session_id?: string;
    };
    sentAcpInputs.push({
      agent_id: agentId,
      input: payload.input,
      session_id: payload.session_id,
    });
    await route.fulfill(jsonResponse({ status: "ok" }));
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Chat First Team");
  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);

  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect(page.getByText("conversation-chat-1", { exact: true })).toBeVisible();
  await expect(page.getByText("Negotiate scope with leader")).toBeVisible();

  await page.getByRole("button", { name: "Create Run from Preview" }).click();
  await openMainTeamAction(page, "Runs");
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(runId);
  expect(createRunRequests).toHaveLength(1);
  expect(createRunRequests[0]).toMatchObject({
    context_id: "ctx-chat-first-1",
  });
  expect(
    (createRunRequests[0]?.input as { task_id?: string } | undefined)
      ?.task_id
  ).toBe("task-chat-1");

  await selectAgentFromSidebar(page, "Worker Agent");
  const agentInput = page.getByPlaceholder(/Send input|Type a message \(tap Send/);
  await expect(agentInput).toBeVisible();
  await agentInput.fill("Please include migration notes in the final report.");
  await page.getByRole("button", { name: "Send input", exact: true }).click();
  await expect
    .poll(() => sentAcpInputs.length, { timeout: 15_000 })
    .toBe(1);
  expect(sentAcpInputs).toHaveLength(1);
  expect(sentAcpInputs[0]).toMatchObject({
    agent_id: "agent-worker-1",
    input: "Please include migration notes in the final report.",
  });

  await openAdvancedView(page, "Member Console");
  await expect(page.locator(".teams-step-body")).toContainText("a2a_discovery_card");
  await expect(page.locator(".teams-step-body")).not.toContainText("Loading discovery card...");
  await expect(page.locator(".teams-step-body")).toContainText("acp_gemini");

  await openMainTeamAction(page, "Runs");
  await openAdvancedView(page, "Events");
  await expect(page.locator(".teams-event-list")).toContainText(
    "Final deliverable prepared and returned to user."
  );
});

testLocalLlm("team conversation-first integration supports virtual team tiny-tool delivery flow [local-llm]", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const runId = "run-virtual-tool-1";
  let activeRun: TeamRunRecord | null = null;
  const createRunRequests: Array<{ context_id?: string; input?: unknown }> = [];

  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    const url = new URL(request.url());
    const teamId = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/)?.[1] ?? "";
    if (!teamId) {
      await route.fulfill(jsonResponse({ error: "team id missing" }, 400));
      return;
    }
    if (request.method() === "GET") {
      await route.fulfill(
        jsonResponse(activeRun && activeRun.team_id === teamId ? [activeRun] : [])
      );
      return;
    }
    if (request.method() === "POST") {
      const payload = request.postDataJSON() as { context_id?: string; input?: unknown };
      createRunRequests.push(payload);
      activeRun = {
        id: runId,
        team_id: teamId,
        context_id: payload.context_id ?? "ctx-virtual-tool-fallback",
        status: "working",
        input: (payload.input as Record<string, unknown>) ?? {},
        created_at: fixture.now + 900,
        started_at: fixture.now + 901,
        ended_at: null,
      };
      await route.fulfill(jsonResponse(activeRun));
      return;
    }
    await route.fallback();
  });

  await page.route(/\/api\/teams\/runs\/[^/]+$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "run not found" }, 404));
      return;
    }
    await route.fulfill(jsonResponse(activeRun));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/steps$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(/\/api\/teams\/runs\/[^/]+\/snapshot(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    if (!activeRun) {
      await route.fulfill(jsonResponse({ error: "snapshot unavailable" }, 404));
      return;
    }
    const team = fixture.teams.find((item) => item.id === activeRun?.team_id) ?? null;
    await route.fulfill(
      jsonResponse({
        run: activeRun,
        team,
        leader_member_id: team?.spec.leader_member_id ?? "tool-leader",
        members: [],
        steps: [],
        latest_events: [],
        mailbox: {
          pending: 0,
          delivered: 0,
          dead_letter: 0,
          recent_messages: [],
        },
      })
    );
  });

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await gotoTeams(page);
  await createTeamFromModal(page, {
    name: "virtual-tool-team",
    goal: "Conversation-first tiny tool delivery flow.",
  });

  const virtualToolSpec = {
    spec_version: 1,
    entrypoint: "leader_plan",
    leader_member_id: "tool-leader",
    members: [
      { member_id: "tool-leader", role: "leader", model: "codex" },
      { member_id: "tool-worker", role: "worker", model: "gemini" },
    ],
    steps: [
      { step_key: "leader_plan", member_id: "tool-leader", depends_on: [] },
      { step_key: "worker_build_tool", member_id: "tool-worker", depends_on: ["leader_plan"] },
      {
        step_key: "leader_synthesize",
        member_id: "tool-leader",
        depends_on: ["worker_build_tool"],
      },
    ],
  };
  fixture.agents.push(
    {
      id: "tool-leader",
      name: "tool-leader",
      workdir: "/workspace/tool-leader",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 210,
      updated_at: fixture.now + 210,
    },
    {
      id: "tool-worker",
      name: "tool-worker",
      workdir: "/workspace/tool-worker",
      command: "gemini",
      args: ["--acp"],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "running",
      created_at: fixture.now + 211,
      updated_at: fixture.now + 211,
    }
  );
  const virtualToolTeam = fixture.teams.find((team) => team.name === "virtual-tool-team");
  if (!virtualToolTeam) {
    throw new Error("virtual-tool-team was not created");
  }
  virtualToolTeam.spec = virtualToolSpec;
  virtualToolTeam.updated_at += 1;
  await gotoTeams(page);
  await openTeamFromSelector(page, "virtual-tool-team");

  await expect(page.getByRole("heading", { name: "# all", exact: true })).toBeVisible();
  await page
    .getByPlaceholder("Message #all")
    .fill("Please build a tiny JSON CLI with parse and pretty-print commands.");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.locator(".teams-main")).toContainText(
    "Please build a tiny JSON CLI with parse and pretty-print commands."
  );

  await selectPrimaryTeamEntryFromSidebar(page, "Kanban");
  await openKanbanDeveloperTools(page);
  await page.getByRole("button", { name: "Compile Preview", exact: true }).click();
  await expect(page.locator(".teams-step-body")).toContainText("tiny-json-cli");
  await expect(page.locator(".teams-step-body")).toContainText(
    "Define tiny CLI interface"
  );

  await page.getByRole("button", { name: "Create Run from Preview" }).click();
  await expect(page.locator(".teams-run-list .team-item").first()).toContainText(runId);
  expect(createRunRequests).toHaveLength(1);
  expect(
    (createRunRequests[0]?.input as { tool_name?: string } | undefined)?.tool_name
  ).toBe("tiny-json-cli");
  expect(
    (createRunRequests[0]?.input as { objective?: string } | undefined)?.objective
  ).toBe("Build tiny JSON CLI");
});

test("team mailbox IM mode supports conversation focus, unread, auto-follow and advanced controls", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-mailbox";
  const runId = "run-mailbox-1";
  const teamCreatedAt = fixture.now + 100;
  const runCreatedAt = fixture.now + 200;

  fixture.teams.push({
    id: teamId,
    name: "Team Mailbox",
    description: "mailbox im test",
    spec: {
      leader_member_id: "agent-leader-1",
      members: [
        { member_id: "agent-leader-1", role: "leader", model: "codex" },
        { member_id: "agent-worker-1", role: "worker", model: "gemini" },
        { member_id: "agent-worker-2", role: "worker", model: "kimi" },
      ],
      steps: [{ step_key: "leader_plan" }],
    },
    created_at: teamCreatedAt,
    updated_at: teamCreatedAt,
  });

  const runRecord: TeamRunRecord = {
    id: runId,
    team_id: teamId,
    context_id: "ctx-mailbox",
    status: "working",
    input: { prompt: "mailbox test" },
    created_at: runCreatedAt,
    started_at: runCreatedAt + 1,
    ended_at: null,
  };

  const now = fixture.now + 1_000;
  const messages: TeamActorMessageRecord[] = [];
  for (let index = 1; index <= 36; index += 1) {
    const fromWorker = index % 2 === 0;
    messages.push({
      message_id: index,
      run_id: runId,
      from_actor_id: fromWorker ? "agent-worker-1" : "agent-leader-1",
      to_actor_id: fromWorker ? "agent-leader-1" : "agent-worker-1",
      channel: "default",
      transport: "local",
      route: null,
      payload: {
        type: "chat_message",
        text: `worker1-${index}`,
      },
      status: "pending",
      created_at: now + index,
      delivered_at: null,
    });
  }
  messages.push({
    message_id: 80,
    run_id: runId,
    from_actor_id: "agent-worker-2",
    to_actor_id: "agent-leader-1",
    channel: "default",
    transport: "local",
    route: null,
    payload: { type: "chat_message", text: "worker2-unread" },
    status: "pending",
    created_at: now + 80,
    delivered_at: null,
  });
  let nextMessageId = 120;

  const counters = {
    events: 0,
    snapshot: 0,
    inbox: 0,
    send: 0,
  };

  const computePendingInboxCount = (actorId: string): number =>
    messages.filter(
      (message) => message.to_actor_id === actorId && message.status === "pending"
    ).length;

  const buildSnapshot = () => ({
    run: runRecord,
    team: fixture.teams.find((team) => team.id === teamId),
    leader_member_id: "agent-leader-1",
    members: [
      {
        member_id: "agent-leader-1",
        role: "leader",
        model: "codex",
        prompt: "leader",
        skills: ["agenthub-actor-runtime", "team-leader-orchestrator"],
        pending_inbox_count: computePendingInboxCount("agent-leader-1"),
        status: "working",
        latest_step: null,
        session_status: "working",
      },
      {
        member_id: "agent-worker-1",
        role: "worker",
        model: "gemini",
        prompt: "worker-1",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
        pending_inbox_count: computePendingInboxCount("agent-worker-1"),
        status: "working",
        latest_step: null,
        session_status: "idle",
      },
      {
        member_id: "agent-worker-2",
        role: "worker",
        model: "kimi",
        prompt: "worker-2",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
        pending_inbox_count: computePendingInboxCount("agent-worker-2"),
        status: "working",
        latest_step: null,
        session_status: "idle",
      },
    ],
    steps: [],
    latest_events: [
      {
        event_id: 1,
        run_id: runId,
        step_id: null,
        event_type: "run_working",
        ts: now,
        payload: { status: "working" },
      },
    ],
    mailbox: {
      pending: messages.filter((message) => message.status === "pending").length,
      delivered: messages.filter((message) => message.status === "delivered").length,
      dead_letter: 0,
      recent_messages: [...messages].sort((a, b) => a.message_id - b.message_id),
    },
  });

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

  await page.route(new RegExp(`/api/teams/runs/${runId}/events(?:\\?.*)?$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    counters.events += 1;
    await route.fulfill(jsonResponse([]));
  });

  await page.route(new RegExp(`/api/teams/runs/${runId}/steps$`), async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/snapshot(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      counters.snapshot += 1;
      await route.fulfill(jsonResponse(buildSnapshot()));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/inbox(?:\\?.*)?$`),
    async (route, request) => {
      if (request.method() !== "GET") {
        await route.fallback();
        return;
      }
      counters.inbox += 1;
      const url = new URL(request.url());
      const actorId = url.searchParams.get("actor_id") ?? "";
      const includeDelivered = url.searchParams.get("include_delivered") === "true";
      const afterRaw = url.searchParams.get("after_id");
      const afterId = afterRaw ? Number(afterRaw) : null;
      const list = messages
        .filter((message) => message.to_actor_id === actorId)
        .filter((message) => includeDelivered || message.status !== "delivered")
        .filter((message) => (afterId == null ? true : message.message_id > afterId))
        .sort((left, right) => left.message_id - right.message_id);
      await route.fulfill(jsonResponse(list));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/send$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      counters.send += 1;
      const payload = request.postDataJSON() as {
        from_actor_id: string;
        to_actor_id: string;
        channel?: string;
        transport?: "local" | "remote";
        route?: Record<string, unknown> | null;
        payload: unknown;
      };
      const created: TeamActorMessageRecord = {
        message_id: nextMessageId,
        run_id: runId,
        from_actor_id: payload.from_actor_id,
        to_actor_id: payload.to_actor_id,
        channel: payload.channel ?? "default",
        transport: payload.transport ?? "local",
        route: payload.route ?? null,
        payload: payload.payload,
        status: "pending",
        created_at: now + nextMessageId,
        delivered_at: null,
      };
      nextMessageId += 1;
      messages.push(created);
      await route.fulfill(jsonResponse(created));
    }
  );

  await page.route(
    new RegExp(`/api/teams/runs/${runId}/messages/\\d+/ack$`),
    async (route, request) => {
      if (request.method() !== "POST") {
        await route.fallback();
        return;
      }
      const messageIdMatch = request.url().match(/messages\/(\d+)\/ack$/);
      const messageId = Number(messageIdMatch?.[1] ?? "0");
      const message = messages.find((item) => item.message_id === messageId);
      if (!message) {
        await route.fulfill(jsonResponse({ error: "message not found" }, 404));
        return;
      }
      message.status = "delivered";
      message.delivered_at = now + messageId;
      await route.fulfill(jsonResponse(message));
    }
  );

  await page.route(/\/api\/agents\/[^/]+\/events(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    await route.fulfill(jsonResponse([]));
  });

  const unreadFor = async (memberId: string): Promise<number> => {
    const badge = page
      .locator(".teams-chat-members .team-item", { hasText: `${memberId} (` })
      .locator(".teams-member-unread");
    if ((await badge.count()) === 0) {
      return 0;
    }
    const label = await badge.innerText();
    const match = label.match(/unread=(\d+)/);
    return match ? Number(match[1]) : 0;
  };

  await enableDeveloperMode(page);
  await gotoTeams(page);
  await openTeamFromSelector(page, "Team Mailbox");
  await openAdvancedView(page, "Execution Mailbox");
  await expect(page.locator(".teams-chat-shell")).toBeVisible();

  const unreadWorker2Before = await unreadFor("Worker Agent Two");
  expect(unreadWorker2Before).toBeGreaterThan(0);

  await page
    .locator(".teams-chat-members .team-item", { hasText: "Worker Agent (worker)" })
    .click();
  await expect(page.locator(".teams-chat-head")).toContainText(
    "Leader Agent → Worker Agent"
  );
  await page.getByRole("button", { name: "Jump to bottom" }).click();
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=on");
  await expect.poll(async () => unreadFor("Worker Agent")).toBe(0);
  expect(await unreadFor("Worker Agent Two")).toBeGreaterThan(0);

  await page.locator(".teams-chat-messages").evaluate((element) => {
    const target = element as HTMLElement;
    target.scrollTop = 0;
    target.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await expect(page.locator(".teams-chat-head")).toContainText("auto_follow=off");

  await page
    .locator(".teams-chat-members .team-item", { hasText: "Worker Agent Two (worker)" })
    .click();
  await expect(page.locator(".teams-chat-head")).toContainText(
    "Leader Agent → Worker Agent Two"
  );

  const eventsBeforePolling = counters.events;
  const snapshotBeforePolling = counters.snapshot;
  const inboxBeforePolling = counters.inbox;
  await page.waitForTimeout(4500);
  expect(counters.events).toBe(eventsBeforePolling);
  expect(counters.snapshot).toBeGreaterThan(snapshotBeforePolling);
  expect(counters.inbox).toBeGreaterThan(inboxBeforePolling);

  await openAdvancedView(page, "Debug");
  await page.getByRole("button", { name: "Mailbox Raw" }).click();
  await expect(page.getByRole("heading", { name: "Send Message (JSON)" })).toBeVisible();

  const advancedPanel = page.locator(".teams-message-advanced .teams-message-panel").first();
  await advancedPanel.getByPlaceholder("from_actor_id").fill("agent-leader-1");
  await advancedPanel.getByPlaceholder("to_actor_id").fill("agent-worker-2");
  await advancedPanel
    .getByPlaceholder("payload JSON")
    .fill('{"type":"chat_message","text":"advanced-mailbox-ping"}');
  await advancedPanel.getByRole("button", { name: "Send Message" }).click();

  await openMainTeamAction(page, "Runs");
  await openAdvancedView(page, "Overview");
  await page
    .locator(".teams-member-list .team-member-row", { hasText: "Worker Agent Two (worker)" })
    .click();
  await expect(page.locator(".teams-chat-messages")).toContainText("advanced-mailbox-ping");
  expect(counters.send).toBeGreaterThan(0);
});

test("team list supports deleting selected team", async ({ page }) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push(
    {
      id: "team-delete-a",
      name: "Team Delete A",
      description: "first team",
      spec: {
        leader_member_id: "agent-leader-1",
        members: [{ member_id: "agent-leader-1", role: "leader", model: "codex" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now,
      updated_at: fixture.now,
    },
    {
      id: "team-delete-b",
      name: "Team Delete B",
      description: "second team",
      spec: {
        leader_member_id: "agent-worker-1",
        members: [{ member_id: "agent-worker-1", role: "leader", model: "gemini" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now + 1,
      updated_at: fixture.now + 1,
    }
  );

  await gotoTeams(page);
  await expect(page.locator(".team-item", { hasText: "Team Delete A" })).toBeVisible();
  await expect(page.locator(".team-item", { hasText: "Team Delete B" })).toBeVisible();
  await openTeamFromSelector(page, "Team Delete A");
  await openMainTeamAction(page, "Runs");
  await expect(page.locator(".teams-main").getByText("Team Delete A", { exact: true })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete Team" }).click();

  await expect(page.getByRole("heading", { name: "Team Selector" })).toBeVisible();
  await expect(page.locator(".team-item", { hasText: "Team Delete A" })).toHaveCount(0);
  await expect(page.locator(".team-item", { hasText: "Team Delete B" })).toBeVisible();
});

test("team run list keeps per-team filters and uses before_created_at cursor paging", async ({
  page,
}) => {
  const fixture = await mockTeamPageApis(page);
  fixture.teams.push(
    {
      id: "team-a",
      name: "Team A",
      description: "first team",
      spec: {
        leader_member_id: "agent-leader-1",
        members: [{ member_id: "agent-leader-1", role: "leader", model: "codex" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now,
      updated_at: fixture.now,
    },
    {
      id: "team-b",
      name: "Team B",
      description: "second team",
      spec: {
        leader_member_id: "agent-worker-1",
        members: [{ member_id: "agent-worker-1", role: "leader", model: "gemini" }],
        steps: [{ step_key: "leader_plan" }],
      },
      created_at: fixture.now + 1,
      updated_at: fixture.now + 1,
    }
  );

  const runQueries: Array<{ teamId: string; status: string; before: number | null }> = [];
  await page.route(/\/api\/teams\/[^/]+\/runs(?:\?.*)?$/, async (route, request) => {
    if (request.method() !== "GET") {
      await route.fallback();
      return;
    }
    const url = new URL(request.url());
    const teamIdMatch = url.pathname.match(/\/api\/teams\/([^/]+)\/runs$/);
    const teamId = teamIdMatch?.[1] ?? "";
    const status = url.searchParams.get("status") ?? "all";
    const beforeRaw = url.searchParams.get("before_created_at");
    const before = beforeRaw == null ? null : Number(beforeRaw);
    runQueries.push({ teamId, status, before });

    let payload: TeamRunRecord[] = [];
    if (teamId === "team-a" && status === "all") {
      payload = [buildTeamRun("team-a", "submitted", 500, 1)];
    } else if (teamId === "team-a" && status === "working" && before == null) {
      payload = Array.from({ length: 50 }, (_, index) =>
        buildTeamRun("team-a", "working", 300 - index, index)
      );
    } else if (teamId === "team-a" && status === "working" && before === 251) {
      payload = [buildTeamRun("team-a", "working", 250, 999)];
    } else if (teamId === "team-b" && status === "all") {
      payload = [buildTeamRun("team-b", "submitted", 450, 1)];
    } else if (teamId === "team-b" && status === "failed") {
      payload = [buildTeamRun("team-b", "failed", 400, 2)];
    }

    await route.fulfill(jsonResponse(payload));
  });

  await gotoTeams(page);
  await openTeamFromSelector(page, "Team A");
  await openMainTeamAction(page, "Runs");

  const runFilter = page.getByLabel("Run status filter");
  await expect(runFilter).toHaveValue("all");

  await runFilter.selectOption("working");
  await expect(runFilter).toHaveValue("working");
  const loadMoreRunsButton = page.getByRole("button", { name: "Load More" });
  await expect(loadMoreRunsButton).toBeEnabled();
  await loadMoreRunsButton.click();

  await expect
    .poll(() =>
      runQueries.some(
        (query) =>
          query.teamId === "team-a" &&
          query.status === "working" &&
          query.before === 251
      )
    )
    .toBe(true);

  await openTeamFromSelector(page, "Team B");
  await expect(runFilter).toHaveValue("all");
  await runFilter.selectOption("failed");
  await expect(runFilter).toHaveValue("failed");

  await openTeamFromSelector(page, "Team A");
  await expect(runFilter).toHaveValue("working");

  await openTeamFromSelector(page, "Team B");
  await expect(runFilter).toHaveValue("failed");
  await expect(page.getByRole("alert")).toHaveCount(0);
});
