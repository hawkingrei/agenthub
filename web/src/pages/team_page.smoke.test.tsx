// @vitest-environment jsdom
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

vi.mock("./team/use_team_runtime_effects", () => ({
  useTeamRuntimeEffects: () => undefined,
}));

vi.mock("./team/use_team_conversation_actions", () => ({
  useTeamConversationActions: () => ({
    refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
    sendTaskMessage: vi.fn().mockResolvedValue(undefined),
  }),
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
    listTeamTasks.mockClear();
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
    expect(markup).toContain("Select a team");
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
});
