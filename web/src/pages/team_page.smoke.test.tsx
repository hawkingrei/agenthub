// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { getRuntimeDefaults } = vi.hoisted(() => ({
  getRuntimeDefaults: vi.fn().mockResolvedValue({ default_worktree_root: "/tmp/worktrees" }),
}));

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      getRuntimeDefaults,
    },
  };
});

vi.mock("./team/use_team_actions", () => ({
  useTeamActions: () => ({
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
  }),
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
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    getRuntimeDefaults.mockClear();
    window.history.pushState({}, "", "/teams");
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("renders the selector route without crashing", async () => {
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
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain("Team Selector");
    expect(container.textContent).toContain("Select a team");
    expect(getRuntimeDefaults).toHaveBeenCalledWith("token");
  });
});
