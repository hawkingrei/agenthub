// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  createTeamChannel,
  deleteTeamChannel,
  getRuntimeDefaults,
  getTeamPromptDefaults,
  getTeamRuntime,
  getTeamSharedThread,
  getTeamTask,
  listTeamChannels,
  listTeamTasks,
  sidebarPropsSpy,
  teamPageFixture,
  useMediaQueryMock,
} = vi.hoisted(() => ({
  createTeamChannel: vi.fn(),
  deleteTeamChannel: vi.fn().mockResolvedValue(undefined),
  getRuntimeDefaults: vi.fn().mockResolvedValue({ default_worktree_root: "/tmp/worktrees" }),
  getTeamPromptDefaults: vi.fn().mockResolvedValue({
    coordinator_prompt: "coordinator-default-prompt",
    worker_prompt: "worker-default-prompt",
  }),
  getTeamRuntime: vi.fn().mockResolvedValue({
    team_id: "team-1",
    team_name: "Team One",
    status: "stopped",
    members: [],
  }),
  getTeamSharedThread: vi.fn(),
  getTeamTask: vi.fn(),
  listTeamChannels: vi.fn().mockResolvedValue([]),
  listTeamTasks: vi.fn().mockResolvedValue([]),
  sidebarPropsSpy: vi.fn(),
  teamPageFixture: {
    teams: [] as Array<Record<string, unknown>>,
    agents: [] as Array<Record<string, unknown>>,
  },
  useMediaQueryMock: vi.fn(() => false),
}));

vi.mock("@mantine/hooks", () => ({
  useMediaQuery: useMediaQueryMock,
}));

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      createTeamChannel,
      deleteTeamChannel,
      getRuntimeDefaults,
      getTeamPromptDefaults,
      getTeamRuntime,
      getTeamSharedThread,
      getTeamTask,
      listTeamChannels,
      listTeamTasks,
    },
  };
});

vi.mock("./team/use_team_actions", () => ({
  useTeamActions: (options: {
    setTeams: React.Dispatch<React.SetStateAction<Array<Record<string, unknown>>>>;
    setAgents: React.Dispatch<React.SetStateAction<Array<Record<string, unknown>>>>;
    onTeamsRefreshSettled?: () => void;
  }) => {
    const { setAgents, setTeams } = options;
    const onTeamsRefreshSettled = options.onTeamsRefreshSettled;
    React.useEffect(() => {
      setTeams(teamPageFixture.teams);
      setAgents(teamPageFixture.agents);
      onTeamsRefreshSettled?.();
    }, [onTeamsRefreshSettled, setAgents, setTeams]);
    return {
      refreshAgents: vi.fn().mockResolvedValue(undefined),
      refreshTeams: vi.fn().mockResolvedValue(undefined),
      refreshRun: vi.fn().mockResolvedValue(undefined),
      refreshTeamRuns: vi.fn().mockResolvedValue(undefined),
      refreshSteps: vi.fn().mockResolvedValue(undefined),
      refreshEvents: vi.fn().mockResolvedValue(undefined),
      refreshSnapshot: vi.fn().mockResolvedValue(undefined),
      loadInbox: vi.fn().mockResolvedValue(undefined),
      loadMemberEvents: vi.fn().mockResolvedValue(undefined),
      onCreateRun: vi.fn().mockResolvedValue(undefined),
      onLoadRunById: vi.fn().mockResolvedValue(undefined),
      onRefreshRuns: vi.fn().mockResolvedValue(undefined),
      onLoadMoreRuns: vi.fn().mockResolvedValue(undefined),
      onCancelRun: vi.fn().mockResolvedValue(undefined),
      onResumeRun: vi.fn().mockResolvedValue(undefined),
      onRestartRun: vi.fn().mockResolvedValue(undefined),
    };
  },
}));

vi.mock("./team/use_team_step_actions", () => ({
  useTeamStepActions: () => ({
    onSubmitStep: vi.fn().mockResolvedValue(undefined),
    onApplyStepAction: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("./team/use_team_mailbox_actions", () => ({
  useTeamMailboxActions: () => ({
    onSendChatMessage: vi.fn().mockResolvedValue(undefined),
    onSendMessage: vi.fn().mockResolvedValue(undefined),
    onRefreshInbox: vi.fn().mockResolvedValue(undefined),
    onAcceptMessage: vi.fn().mockResolvedValue(undefined),
    onAcceptVisibleMessages: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("./team/use_team_conversation_effects", () => ({
  useTeamConversationEffects: () => undefined,
}));

vi.mock("./team/use_team_member_agent_backfill_effect", () => ({
  useTeamMemberAgentBackfillEffect: () => undefined,
}));

vi.mock("./team/use_team_mailbox_lifecycle_effects", () => ({
  useTeamMailboxLifecycleEffects: () => undefined,
}));

vi.mock("./team/use_team_run_lifecycle_effects", () => ({
  useTeamRunLifecycleEffects: () => undefined,
}));

vi.mock("./team/use_team_runtime_effects", () => ({
  useTeamRuntimeEffects: () => undefined,
}));

vi.mock("./team/use_team_conversation_actions", () => ({
  useTeamConversationActions: () => ({
    refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
    sendTaskMessage: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("./team/TeamWorkbenchContainer", () => ({
  TeamWorkbenchContainer: () => <div data-testid="team-workbench" />,
}));

vi.mock("./team/TeamSidebarContainer", () => ({
  TeamSidebarContainer: (props: Record<string, unknown>) => {
    sidebarPropsSpy(props);
    return (
      <div data-testid="team-sidebar">
        <button
          type="button"
          onClick={() =>
            (props.onSelectAgentWorkspace as (memberId: string, tab: string) => void)(
              "worker-1",
              "mailbox"
            )
          }
        >
          Open member mailbox
        </button>
        <button
          type="button"
          onClick={() => (props.navigateToTeamDetail as (teamId: string) => void)("team-2")}
        >
          Switch team
        </button>
        <button
          type="button"
          onClick={() => (props.onSelectChannel as (channelId: string) => void)("review")}
        >
          Select review channel
        </button>
        <button
          type="button"
          onClick={() =>
            void (props.onCreateChannel as (payload: {
              channelId: string;
              description: string;
            }) => Promise<void>)({
              channelId: "planning",
              description: "Planning lane",
            })
          }
        >
          Create planning channel
        </button>
        <button
          type="button"
          onClick={() => void (props.onDeleteChannel as (channelId: string) => Promise<void>)("review")}
        >
          Delete review channel
        </button>
      </div>
    );
  },
}));

import { TeamPage } from "./team_page";
import {
  buildTeamChannelPath,
  buildTeamChannelThreadPath,
  buildTeamMemberWorkspacePath,
  buildTeamWorkspaceLensPath,
} from "./team/team_route_helpers";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia;
}

if (typeof globalThis.ResizeObserver !== "function") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  } as typeof ResizeObserver;
}

function buildTeam(id: string, name: string) {
  return {
    id,
    name,
    description: "Mission",
    spec: {
      spec_version: 1,
      coordinator_member_id: "coordinator",
      entrypoint: "coordinator_plan",
      steps: [],
      members: [
        {
          member_id: "coordinator",
          role: "coordinator",
          prompt: "Plan",
        },
        {
          member_id: "worker-1",
          role: "worker",
          prompt: "Work",
        },
      ],
    },
    created_at: 1,
    updated_at: 1,
  };
}

function buildSharedThreadDetail(teamId: string) {
  return {
    task: {
      id: `task-${teamId}`,
      team_id: teamId,
      title: "all",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { bootstrap_kind: "shared_thread" },
      created_at: 1,
      updated_at: 1,
    },
    conversation: {
      id: `conv-${teamId}`,
      team_id: teamId,
      task_id: `task-${teamId}`,
      mode: "group_chat",
      topic: "all",
      created_at: 1,
      updated_at: 1,
    },
    latest_run: null,
  };
}

function buildChannelTaskDetail(teamId: string, channelId: string, taskId: string) {
  return {
    task: {
      id: taskId,
      team_id: teamId,
      title: channelId,
      status: "open",
      created_by_actor_id: "user:user-1",
      assigned_member_id: null,
      context: { bootstrap_kind: "team_channel", channel_id: channelId },
      created_at: 1,
      updated_at: 1,
    },
    conversation: {
      id: `conv-${channelId}`,
      team_id: teamId,
      task_id: taskId,
      mode: "group_chat",
      topic: channelId,
      created_at: 1,
      updated_at: 1,
    },
    latest_run: null,
  };
}

async function flushEffects() {
  await Promise.resolve();
  await Promise.resolve();
}

async function renderTeamPage(routePathname = "/workspace/teams/team-1", routeSearch = "") {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root: Root = createRoot(container);
  window.history.replaceState({}, "", `${routePathname}${routeSearch}`);
  await act(async () => {
    root.render(
      <MantineProvider env="test">
        <TeamPage
          auth={{
            token: "token",
            userId: "user-1",
            username: "root",
            role: "root",
          }}
          token="token"
          onLogout={() => {}}
          developerMode={false}
          routePathname={routePathname}
          routeTeamId="team-1"
          routeSearch={routeSearch}
        />
      </MantineProvider>
    );
    await flushEffects();
  });
  return {
    container,
    cleanup: () => {
      act(() => {
        root.unmount();
      });
      container.remove();
    },
  };
}

function clickButton(container: HTMLElement, label: string) {
  const button = Array.from(container.querySelectorAll("button")).find((node) =>
    node.textContent?.includes(label)
  ) as HTMLButtonElement | undefined;
  if (!button) {
    throw new Error(`button not found: ${label}`);
  }
  button.click();
}

describe("TeamPage navigation glue", () => {
  beforeEach(() => {
    createTeamChannel.mockReset();
    createTeamChannel.mockResolvedValue({
      team_id: "team-1",
      channel_id: "planning",
      task_id: "task-planning",
      conversation_id: "conv-planning",
      description: "Planning lane",
      created_by_actor_id: "user:user-1",
      created_at: 1,
      updated_at: 1,
    });
    deleteTeamChannel.mockClear();
    getRuntimeDefaults.mockClear();
    getTeamPromptDefaults.mockClear();
    getTeamRuntime.mockClear();
    getTeamSharedThread.mockReset();
    getTeamSharedThread.mockResolvedValue(buildSharedThreadDetail("team-1"));
    getTeamTask.mockReset();
    getTeamTask.mockImplementation((_token: string, teamId: string, taskId: string) => {
      if (taskId === "task-review") {
        return Promise.resolve(buildChannelTaskDetail(teamId, "review", taskId));
      }
      return Promise.reject({ status: 404, message: "missing task" });
    });
    listTeamChannels.mockReset();
    listTeamChannels.mockResolvedValue([
      {
        team_id: "team-1",
        channel_id: "review",
        task_id: "task-review",
        conversation_id: "conv-review",
        description: "Review lane",
        created_by_actor_id: "user:user-1",
        created_at: 1,
        updated_at: 1,
      },
    ]);
    listTeamTasks.mockReset();
    listTeamTasks.mockResolvedValue([]);
    sidebarPropsSpy.mockClear();
    teamPageFixture.teams = [buildTeam("team-1", "Team One"), buildTeam("team-2", "Team Two")];
    teamPageFixture.agents = [];
    useMediaQueryMock.mockReset();
    useMediaQueryMock.mockReturnValue(false);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("routes sidebar member and team selections through canonical paths", async () => {
    const rendered = await renderTeamPage();
    try {
      await act(async () => {
        clickButton(rendered.container, "Open member mailbox");
        await flushEffects();
      });
      expect(window.location.pathname).toBe(
        buildTeamMemberWorkspacePath("team-1", "worker-1", "mailbox")
      );

      await act(async () => {
        clickButton(rendered.container, "Switch team");
        await flushEffects();
      });
      expect(window.location.pathname).toBe(
        buildTeamWorkspaceLensPath("team-2", "channels", "all")
      );
    } finally {
      rendered.cleanup();
    }
  });

  it("routes sidebar channel selection, creation, and deletion through canonical paths", async () => {
    const rendered = await renderTeamPage(buildTeamChannelPath("team-1", "review"));
    try {
      await act(async () => {
        clickButton(rendered.container, "Select review channel");
        await flushEffects();
      });
      expect(window.location.pathname).toBe(buildTeamChannelPath("team-1", "review"));

      await act(async () => {
        clickButton(rendered.container, "Create planning channel");
        await flushEffects();
      });
      expect(createTeamChannel).toHaveBeenCalledWith("token", "team-1", {
        channel_id: "planning",
        description: "Planning lane",
      });
      expect(window.location.pathname).toBe(buildTeamChannelPath("team-1", "planning"));

      await act(async () => {
        clickButton(rendered.container, "Delete review channel");
        await flushEffects();
      });
      expect(deleteTeamChannel).toHaveBeenCalledWith("token", "team-1", "review");
      expect(window.location.pathname).toBe(buildTeamChannelPath("team-1"));
    } finally {
      rendered.cleanup();
    }
  });

  it("drops an unknown channel route after channels load successfully", async () => {
    listTeamChannels.mockResolvedValue([]);
    const rendered = await renderTeamPage(buildTeamChannelPath("team-1", "missing"));
    try {
      await act(async () => {
        await flushEffects();
      });
      expect(window.location.pathname).toBe(buildTeamChannelPath("team-1"));
    } finally {
      rendered.cleanup();
    }
  });

  it("drops an unknown task from a channel route after task detail lookup misses", async () => {
    const rendered = await renderTeamPage(buildTeamChannelPath("team-1", "review"), "?task=missing");
    try {
      await act(async () => {
        await flushEffects();
      });
      expect(getTeamTask).toHaveBeenCalledWith("token", "team-1", "missing");
      expect(window.location.pathname).toBe(buildTeamChannelPath("team-1", "review"));
      expect(window.location.search).toBe("");
    } finally {
      rendered.cleanup();
    }
  });

  it("keeps thread context when canonicalizing a channel-scoped task route", async () => {
    const rendered = await renderTeamPage(buildTeamChannelPath("team-1"), "?task=task-review&thread=17");
    try {
      await act(async () => {
        await flushEffects();
      });
      expect(getTeamTask).toHaveBeenCalledWith("token", "team-1", "task-review");
      expect(window.location.pathname).toBe(buildTeamChannelThreadPath("team-1", "review", 17));
      expect(window.location.search).toBe("");
    } finally {
      rendered.cleanup();
    }
  });
});
