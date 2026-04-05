// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  getRuntimeDefaults,
  getTeamPromptDefaults,
  getTeamRuntime,
  getTeamSharedThread,
  listTeamTasks,
  teamConversationActionsOptionsSpy,
  teamPageFixture,
  useMediaQueryMock,
} = vi.hoisted(() => ({
  getRuntimeDefaults: vi.fn().mockResolvedValue({ default_worktree_root: "/tmp/worktrees" }),
  getTeamPromptDefaults: vi.fn().mockResolvedValue({
    leader_prompt: "leader-default-prompt",
    worker_prompt: "worker-default-prompt",
  }),
  getTeamRuntime: vi.fn().mockResolvedValue({
    team_id: "team-1",
    team_name: "Team One",
    status: "stopped",
    members: [],
  }),
  getTeamSharedThread: vi.fn(),
  listTeamTasks: vi.fn().mockResolvedValue([]),
  teamConversationActionsOptionsSpy: vi.fn(),
  useMediaQueryMock: vi.fn(() => false),
  teamPageFixture: {
    teams: [] as Array<Record<string, unknown>>,
    agents: [] as Array<Record<string, unknown>>,
  },
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
      getRuntimeDefaults,
      getTeamPromptDefaults,
      getTeamRuntime,
      getTeamSharedThread,
      listTeamTasks,
    },
  };
});

vi.mock("./team/use_team_actions", () => ({
  useTeamActions: (options: {
    setTeams: React.Dispatch<React.SetStateAction<Array<Record<string, unknown>>>>;
    setAgents: React.Dispatch<React.SetStateAction<Array<Record<string, unknown>>>>;
  }) => {
    const { setTeams, setAgents } = options;
    React.useEffect(() => {
      setTeams(teamPageFixture.teams);
      setAgents(teamPageFixture.agents);
    }, [setAgents, setTeams]);
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
  useTeamConversationActions: (options: {
    selectedConversation: unknown;
    latestRunForSharedConversation: unknown;
    selectedTeamId: string | null;
  }) => {
    teamConversationActionsOptionsSpy({
      selectedConversation: options.selectedConversation,
      latestRunForSharedConversation: options.latestRunForSharedConversation,
      selectedTeamId: options.selectedTeamId,
    });
    return {
      refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
      sendTaskMessage: vi.fn().mockResolvedValue(undefined),
    };
  },
}));

vi.mock("../components/workbench_header_menu", () => ({
  WorkbenchHeaderMenu: () => <div data-testid="workbench-header-menu">Menu</div>,
}));

import { TeamPage } from "./team_page";

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

describe("TeamPage smoke render", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn> | null = null;

  const flushEffects = async () => {
    await Promise.resolve();
    await Promise.resolve();
  };

  const changeInputValue = async (element: HTMLInputElement, value: string) => {
    await act(async () => {
      const prototype = Object.getPrototypeOf(element) as { value?: unknown };
      const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");
      if (descriptor?.set) {
        descriptor.set.call(element, value);
      } else {
        element.value = value;
      }
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
      await flushEffects();
    });
  };

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    getRuntimeDefaults.mockClear();
    getTeamPromptDefaults.mockClear();
    getTeamRuntime.mockClear();
    getTeamSharedThread.mockClear();
    listTeamTasks.mockClear();
    teamConversationActionsOptionsSpy.mockClear();
    useMediaQueryMock.mockReset();
    useMediaQueryMock.mockReturnValue(false);
    teamPageFixture.teams = [];
    teamPageFixture.agents = [];
    window.history.pushState({}, "", "/teams");
  });

  afterEach(() => {
    consoleErrorSpy?.mockRestore();
    consoleErrorSpy = null;
  });

  it("renders the selector route without crashing", () => {
    const markup = renderToStaticMarkup(
        <MantineProvider>
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
            routeTeamId={null}
          />
        </MantineProvider>
      );

    expect(markup).toContain("Team Selector");
    expect(markup).toContain("Choose a team");
    expect(markup).toContain("No teams yet.");
    expect(markup).not.toContain("Workspace Flow");
    expect(markup).not.toContain("Mission before staffing");
  });

  it("renders a team detail route without crashing", () => {
    const markup = renderToStaticMarkup(
        <MantineProvider>
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
            routeTeamId="team-1"
          />
        </MantineProvider>
      );

    expect(markup).toContain("Loading team workspace");
    expect(markup).not.toContain("This team is unavailable");
  });

  it("renders team detail content immediately when routing from selector to an already-loaded team", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [],
      },
      created_at: 1,
      updated_at: 1,
    });

    teamPageFixture.teams = [buildTeam("team-1", "Team One")];

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId={null}
            />
          </MantineProvider>
        );
      });

      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
      });

      expect(container.textContent).toContain("No agents have joined this team yet.");
      expect(container.textContent).not.toContain("This team is unavailable.");
      expect(container.textContent).not.toContain("Loading team workspace...");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("clears the selector filter when returning from team detail to the selector route", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [],
      },
      created_at: 1,
      updated_at: 1,
    });

    teamPageFixture.teams = [buildTeam("team-a", "Team A"), buildTeam("team-b", "Team B")];

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId={null}
            />
          </MantineProvider>
        );
        await flushEffects();
      });

      const filterInput = container.querySelector(
        'input[aria-label="Filter teams"]'
      ) as HTMLInputElement | null;
      expect(filterInput).not.toBeNull();
      await changeInputValue(filterInput!, "Team B");
      expect(container.textContent).not.toContain("Team A");
      expect(container.textContent).toContain("Team B");

      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-b"
            />
          </MantineProvider>
        );
        await flushEffects();
      });

      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId={null}
            />
          </MantineProvider>
        );
        await flushEffects();
      });

      expect(container.textContent).toContain("Team A");
      expect(container.textContent).toContain("Team B");
      const resetFilterInput = container.querySelector(
        'input[aria-label="Filter teams"]'
      ) as HTMLInputElement | null;
      expect(resetFilterInput?.value).toBe("");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("ignores stale shared-thread responses after switching teams", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: null,
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [{ member_id: "leader", role: "leader", prompt: "Plan", skills: [] }],
      },
      created_at: 1,
      updated_at: 1,
    });
    const buildSharedThreadDetail = (teamId: string, taskId: string, runId: string) => ({
      task: {
        id: taskId,
        team_id: teamId,
        title: "all",
        status: "in_progress",
        created_by_actor_id: "leader",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 1,
      },
      conversation: {
        id: `conv-${taskId}`,
        team_id: teamId,
        task_id: taskId,
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: {
        id: runId,
        team_id: teamId,
        context_id: `ctx-${runId}`,
        status: "working",
        input: {},
        created_at: 1,
        started_at: null,
        ended_at: null,
      },
    });
    const deferred = <T,>() => {
      let resolve!: (value: T) => void;
      let reject!: (reason?: unknown) => void;
      const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      });
      return { promise, resolve, reject };
    };
    const flushEffects = async () => {
      await Promise.resolve();
      await Promise.resolve();
    };

    teamPageFixture.teams = [buildTeam("team-1", "Team One"), buildTeam("team-2", "Team Two")];
    const teamOneRequest = deferred<ReturnType<typeof buildSharedThreadDetail>>();
    const teamTwoRequest = deferred<ReturnType<typeof buildSharedThreadDetail>>();
    getTeamSharedThread.mockImplementation((_token: string, teamId: string) => {
      if (teamId === "team-1") {
        return teamOneRequest.promise;
      }
      if (teamId === "team-2") {
        return teamTwoRequest.promise;
      }
      return Promise.reject(new Error(`unexpected team ${teamId}`));
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
        await flushEffects();
      });

      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-2"
            />
          </MantineProvider>
        );
        await flushEffects();
      });

      await act(async () => {
        teamTwoRequest.resolve(buildSharedThreadDetail("team-2", "task-team-2", "run-team-2"));
        await flushEffects();
      });

      const afterTeamTwo = teamConversationActionsOptionsSpy.mock.calls.at(-1)?.[0] as {
        selectedConversation?: { id?: string };
        latestRunForSharedConversation?: { id?: string };
        selectedTeamId?: string | null;
      };
      expect(afterTeamTwo.selectedTeamId).toBe("team-2");
      expect(afterTeamTwo.selectedConversation?.id).toBe("task-team-2");
      expect(afterTeamTwo.latestRunForSharedConversation?.id).toBe("run-team-2");

      await act(async () => {
        teamOneRequest.resolve(buildSharedThreadDetail("team-1", "task-team-1", "run-team-1"));
        await flushEffects();
      });

      const finalOptions = teamConversationActionsOptionsSpy.mock.calls.at(-1)?.[0] as {
        selectedConversation?: { id?: string };
        latestRunForSharedConversation?: { id?: string };
        selectedTeamId?: string | null;
      };
      expect(finalOptions.selectedTeamId).toBe("team-2");
      expect(finalOptions.selectedConversation?.id).toBe("task-team-2");
      expect(finalOptions.latestRunForSharedConversation?.id).toBe("run-team-2");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("shows the workspace pane by default on compact detail routes", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [{ member_id: "leader", role: "leader", prompt: "Plan" }],
      },
      created_at: 1,
      updated_at: 1,
    });

    useMediaQueryMock.mockReturnValue(true);
    teamPageFixture.teams = [buildTeam("team-1", "Team One")];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "leader",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 1,
      },
      conversation: {
        id: "conv-task-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: null,
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(container.querySelector('[aria-label="Show teams panel"]')).not.toBeNull();
      expect(container.querySelector("h1")).toBeNull();
      expect(container.textContent).toContain("# all");
      expect(container.textContent).toContain("Shared channel for human requests");
      expect(container.textContent).not.toContain("Toggle agents section");
      const buttonLabels = Array.from(container.querySelectorAll("button")).map((button) =>
        button.textContent?.replace(/\s+/g, " ").trim() ?? ""
      );
      expect(buttonLabels).toContain("More");
      expect(buttonLabels).not.toContain("Runs");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("keeps the desktop workbench as a fixed-height shell with internal scrolling", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [{ member_id: "leader", role: "leader", prompt: "Plan" }],
      },
      created_at: 1,
      updated_at: 1,
    });

    useMediaQueryMock.mockReturnValue(false);
    teamPageFixture.teams = [buildTeam("team-1", "Team One")];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "leader",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 1,
      },
      conversation: {
        id: "conv-task-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: null,
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
        await Promise.resolve();
        await Promise.resolve();
      });

      const workbench = container.querySelector('[data-team-surface="workbench"]');
      expect(workbench?.className).toContain("overflow-hidden");
      expect(workbench?.className).not.toContain("overflow-y-auto");

      const layout = container.querySelector(".teams-layout");
      expect(layout?.className).not.toContain("items-start");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("collapses the compact sidebar after selecting # all", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [{ member_id: "leader", role: "leader", prompt: "Plan" }],
      },
      created_at: 1,
      updated_at: 1,
    });

    useMediaQueryMock.mockReturnValue(true);
    teamPageFixture.teams = [buildTeam("team-1", "Team One")];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "leader",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 1,
      },
      conversation: {
        id: "conv-task-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: null,
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
        await Promise.resolve();
        await Promise.resolve();
      });

      const showTeamsPanelButton = container.querySelector(
        '[aria-label="Show teams panel"]'
      ) as HTMLButtonElement | null;
      expect(showTeamsPanelButton).not.toBeNull();

      await act(async () => {
        showTeamsPanelButton?.click();
        await Promise.resolve();
      });

      const showWorkbenchButton = container.querySelector(
        '[aria-label="Show workbench"]'
      ) as HTMLButtonElement | null;
      expect(showWorkbenchButton).not.toBeNull();

      const allButtons = Array.from(container.querySelectorAll("button")).filter(
        (button) => button.textContent?.includes("# all")
      ) as HTMLButtonElement[];
      expect(allButtons.length).toBeGreaterThan(0);

      await act(async () => {
        allButtons[0]?.click();
        await Promise.resolve();
      });

      expect(container.querySelector('[aria-label="Show teams panel"]')).not.toBeNull();
      expect(container.querySelector('[aria-label="Show workbench"]')).toBeNull();
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("collapses the compact sidebar after selecting Kanban", async () => {
    const buildTeam = (id: string, name: string) => ({
      id,
      name,
      description: "Mission",
      spec: {
        spec_version: 1,
        leader_member_id: "leader",
        entrypoint: "leader_plan",
        steps: [],
        members: [{ member_id: "leader", role: "leader", prompt: "Plan" }],
      },
      created_at: 1,
      updated_at: 1,
    });

    useMediaQueryMock.mockReturnValue(true);
    teamPageFixture.teams = [buildTeam("team-1", "Team One")];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "leader",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 1,
      },
      conversation: {
        id: "conv-task-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: null,
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <MantineProvider>
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
              routeTeamId="team-1"
            />
          </MantineProvider>
        );
        await Promise.resolve();
        await Promise.resolve();
      });

      const showTeamsPanelButton = container.querySelector(
        '[aria-label="Show teams panel"]'
      ) as HTMLButtonElement | null;
      expect(showTeamsPanelButton).not.toBeNull();

      await act(async () => {
        showTeamsPanelButton?.click();
        await Promise.resolve();
      });

      const kanbanButtons = Array.from(container.querySelectorAll("button")).filter(
        (button) => button.textContent?.includes("Kanban")
      ) as HTMLButtonElement[];
      expect(kanbanButtons.length).toBeGreaterThan(0);

      await act(async () => {
        kanbanButtons[0]?.click();
        await Promise.resolve();
      });

      expect(container.querySelector('[aria-label="Show teams panel"]')).not.toBeNull();
      expect(container.querySelector('[aria-label="Show workbench"]')).toBeNull();
      expect(container.textContent).toContain("Canonical Kanban");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

});
