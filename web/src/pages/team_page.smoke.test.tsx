// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { getRuntimeDefaults, getTeamRuntime, listTeamTasks, teamPageFixture } = vi.hoisted(() => ({
  getRuntimeDefaults: vi.fn().mockResolvedValue({ default_worktree_root: "/tmp/worktrees" }),
  getTeamRuntime: vi.fn().mockResolvedValue({
    team_id: "team-1",
    team_name: "Team One",
    status: "stopped",
    members: [],
  }),
  listTeamTasks: vi.fn().mockResolvedValue([]),
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

vi.mock("./team/use_team_conversation_actions", () => ({
  useTeamConversationActions: () => ({
    refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
    sendTaskMessage: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("./team_member_acp_panel", () => ({
  TeamMemberAcpPanel: (props: { selectedMemberId: string }) => (
    <div data-testid="team-member-acp-panel">Agent ACP {props.selectedMemberId}</div>
  ),
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
  beforeEach(() => {
    getRuntimeDefaults.mockClear();
    getTeamRuntime.mockClear();
    listTeamTasks.mockClear();
    teamPageFixture.teams = [];
    teamPageFixture.agents = [];
    window.history.pushState({}, "", "/teams");
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
    expect(markup).toContain("Select a team");
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

  it("opens Agent ACP from the team sidebar without falling back to runs", async () => {
    teamPageFixture.teams = [
      {
        id: "team-1",
        name: "Team One",
        description: "Coordinate the active backlog.",
        spec: {
          leader_member_id: "leader-agent",
          members: [{ member_id: "leader-agent", role: "leader" }],
        },
        created_at: 1,
        updated_at: 1,
      },
    ];
    teamPageFixture.agents = [
      {
        id: "leader-agent",
        name: "Leader Agent",
        workdir: "/tmp",
        command: "codex",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: false,
        status: "running",
        created_at: 1,
        updated_at: 1,
      },
    ];

    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);

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

      const leaderAgentButton = Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes("Leader Agent")
      ) as HTMLButtonElement | undefined;
      expect(leaderAgentButton).toBeDefined();

      await act(async () => {
        leaderAgentButton?.dispatchEvent(
          new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await Promise.resolve();
      });

      expect(container.textContent).toContain("Agent ACP leader-agent");
      expect(container.textContent).not.toContain("Go to Runs");
    } finally {
      await act(async () => {
        root.unmount();
      });
      container.remove();
    }
  });
});
