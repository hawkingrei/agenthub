// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  getRuntimeDefaults,
  getTeamPromptDefaults,
  getTeamRuntime,
  getTeamSharedThread,
  getTeamTask,
  listTeamTasks,
  setAgentLoop,
  teamPageFixture,
  updateTeamSpec,
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
  getTeamTask: vi.fn(),
  listTeamTasks: vi.fn().mockResolvedValue([]),
  setAgentLoop: vi.fn().mockResolvedValue({ status: "ok" }),
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
      getTeamSharedThread,
      getTeamTask,
      listTeamTasks,
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
      refreshTeamRuntime: vi.fn().mockResolvedValue(undefined),
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
  TeamMemberAcpPanel: () => <div>Mock agent ACP panel</div>,
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

  beforeEach(() => {
    getRuntimeDefaults.mockClear();
    getTeamPromptDefaults.mockClear();
    getTeamRuntime.mockClear();
    getTeamSharedThread.mockClear();
    getTeamTask.mockClear();
    listTeamTasks.mockClear();
    setAgentLoop.mockClear();
    updateTeamSpec.mockClear();
    useMediaQueryMock.mockReset();
    useMediaQueryMock.mockReturnValue(false);
    teamPageFixture.teams = [];
    teamPageFixture.agents = [];
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
        leader_member_id: "leader",
        entrypoint: "leader_plan",
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

      await clickElement(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Open worker workspace")
        ) as HTMLButtonElement | null
      );
      await clickElement(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Open edit profile")
        ) as HTMLButtonElement | null
      );

      const identityInput = Array.from(document.body.querySelectorAll("input")).find(
        (input) => input.value === "Investigate regressions"
      ) as HTMLInputElement | undefined ?? null;
      const idleTimeoutInput = document.body.querySelector(
        'input[placeholder="900"]'
      ) as HTMLInputElement | null;
      const loopPromptInput = Array.from(document.body.querySelectorAll("textarea")).find(
        (textarea) =>
          textarea.value === "Resume by checking inbox." ||
          textarea.getAttribute("placeholder")?.includes("You have been idle.")
      ) as HTMLTextAreaElement | undefined ?? null;

      expect(identityInput).not.toBeNull();
      expect(idleTimeoutInput).not.toBeNull();
      expect(loopPromptInput).not.toBeNull();

      const loopEnabledInput = document.body.querySelector(
        'input[type="checkbox"]'
      ) as HTMLInputElement | null;
      expect(loopEnabledInput).not.toBeNull();
      if (!loopEnabledInput?.checked) {
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
});
