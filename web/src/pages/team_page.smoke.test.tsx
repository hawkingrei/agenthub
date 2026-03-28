// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  getRuntimeDefaults,
  getTeamRuntime,
  getTeamSharedThread,
  listTeamTasks,
  teamConversationActionsOptionsSpy,
  teamPageFixture,
} = vi.hoisted(() => ({
  getRuntimeDefaults: vi.fn().mockResolvedValue({ default_worktree_root: "/tmp/worktrees" }),
  getTeamRuntime: vi.fn().mockResolvedValue({
    team_id: "team-1",
    team_name: "Team One",
    status: "stopped",
    members: [],
  }),
  getTeamSharedThread: vi.fn(),
  listTeamTasks: vi.fn().mockResolvedValue([]),
  teamConversationActionsOptionsSpy: vi.fn(),
  teamPageFixture: {
    teams: [] as Array<Record<string, unknown>>,
    agents: [] as Array<Record<string, unknown>>,
  },
}));

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      getRuntimeDefaults,
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
    onAckMessage: vi.fn().mockResolvedValue(undefined),
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

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    getRuntimeDefaults.mockClear();
    getTeamRuntime.mockClear();
    getTeamSharedThread.mockClear();
    listTeamTasks.mockClear();
    teamConversationActionsOptionsSpy.mockClear();
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

    expect(markup).toContain("Team");
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
});
