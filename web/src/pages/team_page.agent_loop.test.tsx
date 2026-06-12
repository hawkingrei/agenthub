// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  getRuntimeDefaults,
  getTeamPromptDefaults,
  getTeamRuntime,
  listTeamChannels,
  getTeamSharedThread,
  getTeamTask,
  listTeamTasks,
  loadMemberEventsSpy,
  refreshTeamRuntimeSpy,
  sendInput,
  setAgentLoop,
  teamMemberAcpPanelPropsSpy,
  teamPageFixture,
  updateTeamSpec,
  useMediaQueryMock,
} = vi.hoisted(() => ({
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
  listTeamChannels: vi.fn().mockResolvedValue([]),
  getTeamSharedThread: vi.fn(),
  getTeamTask: vi.fn(),
  listTeamTasks: vi.fn().mockResolvedValue([]),
  loadMemberEventsSpy: vi.fn().mockResolvedValue(undefined),
  refreshTeamRuntimeSpy: vi.fn().mockResolvedValue(undefined),
  sendInput: vi.fn().mockResolvedValue(undefined),
  setAgentLoop: vi.fn().mockResolvedValue({ status: "ok" }),
  teamMemberAcpPanelPropsSpy: vi.fn(),
  updateTeamSpec: vi.fn(),
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
      listTeamChannels,
      getTeamSharedThread,
      getTeamTask,
      listTeamTasks,
      sendInput,
      setAgentLoop,
      updateTeamSpec,
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
      refreshTeamRuntime: refreshTeamRuntimeSpy,
      loadInbox: vi.fn().mockResolvedValue(undefined),
      loadMemberEvents: loadMemberEventsSpy,
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

vi.mock("./team_sidebar", () => ({
  TeamSidebar: ({
    onSelectAgentTab,
  }: {
    onSelectAgentTab: (memberId: string, tab: string) => void;
  }) => (
    <button type="button" onClick={() => onSelectAgentTab("worker-1", "agent_acp")}>
      Open worker workspace
    </button>
  ),
}));

vi.mock("./team/team_workspace_header", () => ({
  TeamWorkspaceHeader: ({
    isAgentWorkspace,
    onOpenTeamMemberEditModal,
  }: {
    isAgentWorkspace: boolean;
    onOpenTeamMemberEditModal: () => void;
  }) =>
    isAgentWorkspace ? (
      <button type="button" onClick={onOpenTeamMemberEditModal}>
        Open edit profile
      </button>
    ) : (
      <div>Workspace Header</div>
    ),
}));

vi.mock("./team_member_acp_panel", () => ({
  TeamMemberAcpPanel: (props: {
    selectedSessionId?: string | null;
    onSendInput?: (text: string, sessionId: string) => Promise<void> | void;
  }) => {
    teamMemberAcpPanelPropsSpy(props);
    return (
      <button
        type="button"
        onClick={() => {
          void props.onSendInput?.("hello from acp", props.selectedSessionId ?? "");
        }}
      >
        Mock agent ACP panel
      </button>
    );
  },
}));

vi.mock("./team/team_page_modals", () => ({
  TeamPageModals: ({
    showTeamMemberEditModal,
    teamMemberEditDraft,
    patchTeamMemberEditDraft,
    onSaveTeamMemberProfile,
  }: {
    showTeamMemberEditModal: boolean;
    teamMemberEditDraft: {
      description: string;
      agent_loop_enabled: boolean;
      agent_loop_idle_seconds: string;
      agent_loop_prompt: string;
    } | null;
    patchTeamMemberEditDraft: (patch: Record<string, unknown>) => void;
    onSaveTeamMemberProfile: () => void;
  }) => {
    if (!showTeamMemberEditModal || !teamMemberEditDraft) {
      return null;
    }
    return (
      <div>
        <input
          placeholder="Short role description exposed on the agent card"
          value={teamMemberEditDraft.description}
          onChange={(event) =>
            patchTeamMemberEditDraft({ description: event.currentTarget.value })
          }
        />
        <input
          type="checkbox"
          checked={teamMemberEditDraft.agent_loop_enabled}
          onChange={(event) =>
            patchTeamMemberEditDraft({ agent_loop_enabled: event.currentTarget.checked })
          }
        />
        <input
          placeholder="900"
          value={teamMemberEditDraft.agent_loop_idle_seconds}
          onChange={(event) =>
            patchTeamMemberEditDraft({
              agent_loop_idle_seconds: event.currentTarget.value,
            })
          }
        />
        <textarea
          placeholder="You have been idle."
          value={teamMemberEditDraft.agent_loop_prompt}
          onChange={(event) =>
            patchTeamMemberEditDraft({ agent_loop_prompt: event.currentTarget.value })
          }
        />
        <button type="button" onClick={onSaveTeamMemberProfile}>
          Save Profile
        </button>
      </div>
    );
  },
}));

vi.mock("./team/TeamWorkbenchContainer", async () => {
  const { useTeamWorkspace } = await vi.importActual<typeof import("./team/team_workspace_context")>(
    "./team/team_workspace_context"
  );
  return {
    TeamWorkbenchContainer: () => {
      const { workbench } = useTeamWorkspace();
      if (!workbench) {
        return null;
      }
      teamMemberAcpPanelPropsSpy({
        selectedMemberId: workbench.selectedAgentWorkspaceMemberId,
        selectedSessionId: workbench.selectedAgentWorkspaceSessionId,
      });
      return (
        <div>
          {workbench.isAgentWorkspace ? (
            <button type="button" onClick={workbench.onOpenTeamMemberEditModal}>
              Open edit profile
            </button>
          ) : null}
          {workbench.tab === "agent_acp" ? (
            <button
              type="button"
              onClick={() => {
                workbench.onSendAgentAcpInput(
                  "hello from acp",
                  workbench.selectedAgentWorkspaceSessionId ?? ""
                );
              }}
            >
              Mock agent ACP panel
            </button>
          ) : null}
        </div>
      );
    },
  };
});

import { TeamPage } from "./team_page";
import { buildTeamWorkspacePath, resolveTeamRoute } from "../app_route_selection";

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

const documentWithFonts = document as Document & {
  fonts?: {
    addEventListener?: (type: string, listener: EventListenerOrEventListenerObject) => void;
    removeEventListener?: (type: string, listener: EventListenerOrEventListenerObject) => void;
  };
};

if (!documentWithFonts.fonts) {
  Object.defineProperty(documentWithFonts, "fonts", {
    configurable: true,
    value: {
      addEventListener: () => {},
      removeEventListener: () => {},
    },
  });
} else {
  documentWithFonts.fonts.addEventListener ??= () => {};
  documentWithFonts.fonts.removeEventListener ??= () => {};
}

describe("TeamPage agent loop profile flow", () => {
  const flushEffects = async () => {
    await Promise.resolve();
    await Promise.resolve();
    await new Promise((resolve) => window.setTimeout(resolve, 0));
  };

  const clickElement = async (element: HTMLElement | null) => {
    expect(element).not.toBeNull();
    await act(async () => {
      element?.click();
      await flushEffects();
    });
  };

  const changeInputValue = async (
    element: HTMLInputElement | HTMLTextAreaElement,
    value: string
  ) => {
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

  async function waitForElement<T>(
    lookup: () => T | null,
    message: string
  ): Promise<T> {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      const element = lookup();
      if (element !== null) {
        return element;
      }
      await act(async () => {
        await flushEffects();
      });
    }
    throw new Error(message);
  }

  function TestTeamPageRouter() {
    const [routeLocation, setRouteLocation] = React.useState(() => ({
      pathname: window.location.pathname,
      search: window.location.search,
    }));
    React.useEffect(() => {
      const onPopState = () => {
        setRouteLocation({
          pathname: window.location.pathname,
          search: window.location.search,
        });
      };
      window.addEventListener("popstate", onPopState);
      return () => {
        window.removeEventListener("popstate", onPopState);
      };
    }, []);
    const teamRoute = resolveTeamRoute(routeLocation.pathname);
    return (
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
          routePathname={routeLocation.pathname}
          routeTeamId={teamRoute?.teamId ?? null}
          routeSearch={routeLocation.search}
        />
      </MantineProvider>
    );
  }

  beforeEach(() => {
    getRuntimeDefaults.mockClear();
    getTeamPromptDefaults.mockClear();
    getTeamRuntime.mockClear();
    listTeamChannels.mockClear();
    getTeamSharedThread.mockClear();
    getTeamTask.mockClear();
    listTeamTasks.mockClear();
    loadMemberEventsSpy.mockClear();
    refreshTeamRuntimeSpy.mockClear();
    sendInput.mockClear();
    setAgentLoop.mockClear();
    teamMemberAcpPanelPropsSpy.mockClear();
    updateTeamSpec.mockClear();
    useMediaQueryMock.mockReset();
    useMediaQueryMock.mockReturnValue(false);
    teamPageFixture.teams = [];
    teamPageFixture.agents = [];
    window.history.pushState({}, "", buildTeamWorkspacePath("team-1"));
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("keeps profile save successful when agent loop update fails afterwards", async () => {
    const team = {
      id: "team-1",
      name: "Team One",
      description: "Mission",
      spec: {
        spec_version: 1,
        coordinator_member_id: "coordinator",
        entrypoint: "coordinator_plan",
        steps: [],
        members: [
          {
            member_id: "worker-1",
            role: "worker",
            description: "Investigate regressions",
            model: "gpt-5.4",
            prompt: "Stay focused on regressions.",
            skills: [],
            runtime: {
              agent_loop_enabled: true,
              agent_loop_idle_seconds: 900,
              agent_loop_prompt: "Resume by checking inbox.",
            },
          },
        ],
      },
      created_at: 1,
      updated_at: 10,
    };
    teamPageFixture.teams = [team];
    teamPageFixture.agents = [
      {
        id: "worker-1",
        name: "Worker One",
        workdir: "/repo",
        command: "codex",
        args: [],
        worktree_mode: "create_worktree",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        agent_loop_enabled: true,
        agent_loop_idle_seconds: 900,
        agent_loop_prompt: "Resume by checking inbox.",
        status: "running",
        created_at: 1,
        updated_at: 2,
      },
    ];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "coordinator",
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
    updateTeamSpec.mockResolvedValue({
      ...team,
      updated_at: 11,
      spec: {
        ...team.spec,
        members: [
          {
            ...team.spec.members[0],
            description: "Investigate regressions and summarize blockers",
            runtime: {
              ...(team.spec.members[0].runtime ?? {}),
              agent_loop_enabled: true,
              agent_loop_idle_seconds: 1200,
              agent_loop_prompt:
                "Resume by checking inbox and summarizing current blockers.",
            },
          },
        ],
      },
    });
    setAgentLoop.mockRejectedValue(new Error("backend unavailable"));

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      window.history.pushState(
        {},
        "",
        buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")
      );
      await act(async () => {
        root.render(<TestTeamPageRouter />);
        await flushEffects();
      });

      const openEditProfileButton = await waitForElement(
        () =>
          Array.from(container.querySelectorAll("button")).find((button) =>
            button.textContent?.includes("Open edit profile")
          ) as HTMLButtonElement | undefined ?? null,
        "open edit profile button missing"
      );
      await clickElement(openEditProfileButton);

      const identityInput = await waitForElement(
        () =>
          document.body.querySelector(
            'input[placeholder="Short role description exposed on the agent card"]'
          ) as HTMLInputElement | null,
        "identity input missing"
      );
      const idleTimeoutInput = await waitForElement(
        () => document.body.querySelector('input[placeholder="900"]') as HTMLInputElement | null,
        "idle timeout input missing"
      );
      const loopPromptInput = await waitForElement(
        () =>
          document.body.querySelector(
            'textarea[placeholder*="You have been idle."]'
          ) as HTMLTextAreaElement | null,
        "loop prompt input missing"
      );

      expect(identityInput.value).toBe("Investigate regressions");
      expect(idleTimeoutInput.value).toBe("900");
      expect(loopPromptInput.value).toBe("Resume by checking inbox.");

      const loopEnabledInput = await waitForElement(
        () => document.body.querySelector('input[type="checkbox"]') as HTMLInputElement | null,
        "agent loop switch missing"
      );
      if (!loopEnabledInput.checked) {
        await clickElement(loopEnabledInput);
      }

      await changeInputValue(
        identityInput!,
        "Investigate regressions and summarize blockers"
      );
      await changeInputValue(idleTimeoutInput!, "1200");
      await changeInputValue(
        loopPromptInput!,
        "Resume by checking inbox and summarizing current blockers."
      );

      await clickElement(
        Array.from(document.body.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Save Profile")
        ) as HTMLButtonElement | null
      );

      expect(updateTeamSpec).toHaveBeenCalledTimes(1);
      expect(updateTeamSpec).toHaveBeenCalledWith(
        "token",
        "team-1",
        expect.objectContaining({
          expected_updated_at: 10,
          spec: expect.objectContaining({
            members: expect.arrayContaining([
              expect.objectContaining({
                member_id: "worker-1",
                description: "Investigate regressions and summarize blockers",
                runtime: expect.objectContaining({
                  agent_loop_enabled: true,
                  agent_loop_idle_seconds: 1200,
                  agent_loop_prompt:
                    "Resume by checking inbox and summarizing current blockers.",
                }),
              }),
            ]),
          }),
        })
      );
      expect(setAgentLoop).toHaveBeenCalledTimes(1);
      expect(setAgentLoop).toHaveBeenCalledWith("token", "worker-1", {
        enabled: true,
        idle_seconds: 1200,
        prompt: "Resume by checking inbox and summarizing current blockers.",
      });
      expect(document.body.textContent).toContain(
        "Agent loop settings were not applied: backend unavailable"
      );
      expect(document.body.textContent).not.toContain("Save Profile");
    } finally {
      await act(async () => {
        root.unmount();
        await flushEffects();
      });
      container.remove();
    }
  });

  it("uses the runtime session for ACP input when the agent record still looks stopped", async () => {
    const team = {
      id: "team-1",
      name: "Team One",
      description: "Mission",
      spec: {
        spec_version: 1,
        coordinator_member_id: "coordinator",
        entrypoint: "coordinator_plan",
        steps: [],
        members: [
          {
            member_id: "worker-1",
            role: "worker",
            description: "Investigate regressions",
            model: "gpt-5.4",
            prompt: "Stay focused on regressions.",
            skills: [],
          },
        ],
      },
      created_at: 1,
      updated_at: 10,
    };
    teamPageFixture.teams = [team];
    teamPageFixture.agents = [
      {
        id: "worker-1",
        name: "Worker One",
        workdir: "/repo",
        command: "codex",
        args: [],
        worktree_mode: "create_worktree",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        status: "stopped",
        created_at: 1,
        updated_at: 2,
      },
    ];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "coordinator",
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
    getTeamRuntime.mockResolvedValue({
      team_id: "team-1",
      team_name: "Team One",
      status: "running",
      members: [
        {
          member_id: "worker-1",
          display_name: "Worker One",
          role: "worker",
          session_id: "runtime-session-stale",
          session_status: "running",
          agent_status: "running",
          card: {
            card_id: "card-worker-1",
            schema_version: "1",
            description: "Investigate regressions",
            capability_tags: [],
          },
        },
      ],
    });
    sendInput
      .mockRejectedValueOnce(
        new Error(
          "agent session mismatch: expected=runtime-session-stale running=runtime-session-running"
        )
      )
      .mockResolvedValueOnce(undefined);

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      window.history.pushState(
        {},
        "",
        buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")
      );
      await act(async () => {
        root.render(<TestTeamPageRouter />);
        await flushEffects();
      });

      await clickElement(
        await waitForElement(
          () =>
            Array.from(container.querySelectorAll("button")).find((button) =>
              button.textContent?.includes("Mock agent ACP panel")
            ) as HTMLButtonElement | undefined ?? null,
          "mock ACP panel missing"
        )
      );

      expect(sendInput).toHaveBeenCalledTimes(2);
      expect(sendInput.mock.calls[0]?.[4]).toBe("runtime-session-stale");
      expect(sendInput.mock.calls[1]?.[4]).toBe("runtime-session-running");
      expect(loadMemberEventsSpy).toHaveBeenCalledWith(
        "replace",
        "runtime-session-running"
      );
      expect(teamMemberAcpPanelPropsSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          selectedSessionId: "runtime-session-running",
        })
      );
    } finally {
      await act(async () => {
        root.unmount();
        await flushEffects();
      });
      container.remove();
    }
  });

  it("uses the member id for ACP history and refreshes events after failed input while the agent record is still backfilling", async () => {
    const team = {
      id: "team-1",
      name: "Team One",
      description: "Mission",
      spec: {
        spec_version: 1,
        coordinator_member_id: "coordinator",
        entrypoint: "coordinator_plan",
        steps: [],
        members: [
          {
            member_id: "worker-1",
            role: "worker",
            description: "Investigate regressions",
            model: "gpt-5.4",
            prompt: "Stay focused on regressions.",
            skills: [],
          },
        ],
      },
      created_at: 1,
      updated_at: 10,
    };
    teamPageFixture.teams = [team];
    teamPageFixture.agents = [];
    getTeamSharedThread.mockResolvedValue({
      task: {
        id: "task-all",
        team_id: "team-1",
        title: "all",
        status: "in_progress",
        created_by_actor_id: "coordinator",
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
    getTeamRuntime.mockResolvedValue({
      team_id: "team-1",
      team_name: "Team One",
      status: "running",
      members: [
        {
          member_id: "worker-1",
          display_name: "Worker One",
          role: "worker",
          session_id: "runtime-session-1",
          session_status: "running",
          agent_status: "running",
          card: {
            card_id: "card-worker-1",
            schema_version: "1",
            description: "Investigate regressions",
            capability_tags: [],
          },
        },
      ],
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      window.history.pushState(
        {},
        "",
        buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")
      );
      await act(async () => {
        root.render(<TestTeamPageRouter />);
        await flushEffects();
      });

      loadMemberEventsSpy.mockClear();
      sendInput.mockRejectedValueOnce(
        new Error("acp command send timed out due to backpressure")
      );
      await clickElement(
        await waitForElement(
          () =>
            Array.from(container.querySelectorAll("button")).find((button) =>
              button.textContent?.includes("Mock agent ACP panel")
            ) as HTMLButtonElement | undefined ?? null,
          "mock ACP panel missing"
        )
      );

      expect(loadMemberEventsSpy).toHaveBeenCalledWith("replace");
      expect(sendInput).toHaveBeenCalledWith(
        "token",
        "worker-1",
        "hello from acp",
        expect.any(String),
        "runtime-session-1"
      );
      expect(teamMemberAcpPanelPropsSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          selectedMemberId: "worker-1",
          selectedSessionId: "runtime-session-1",
        })
      );
    } finally {
      await act(async () => {
        root.unmount();
        await flushEffects();
      });
      container.remove();
    }
  });
});
