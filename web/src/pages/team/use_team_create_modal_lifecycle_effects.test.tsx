// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type AgentRecord, api } from "../../api";
import { DEFAULT_WORKTREE_ROOT, type CreateTeamStage } from "./state";
import { useTeamCreateModalLifecycleEffects } from "./use_team_create_modal_lifecycle_effects";

vi.mock("../../api", () => ({
  api: {
    getRuntimeDefaults: vi.fn(),
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookProps = {
  token: string;
  busy: string | null;
  showCreateTeamModal: boolean;
  leaderMemberId: string;
  teamForgeAgents: AgentRecord[];
  setForgeDefaultWorktreeRoot: (next: string) => void;
  setLeaderMemberId: (next: string) => void;
  setShowCreateTeamModal: (next: boolean) => void;
  setCreateTeamStage: (next: CreateTeamStage) => void;
};

function HookHarness(props: HookProps) {
  useTeamCreateModalLifecycleEffects(props);
  return null;
}

function makeAgent(id: string): AgentRecord {
  return { id } as AgentRecord;
}

async function mountHarness(props: HookProps): Promise<{
  root: Root;
  container: HTMLDivElement;
}> {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(<HookHarness {...props} />);
    await Promise.resolve();
  });
  return { root, container };
}

function cleanupHarness(root: Root, container: HTMLDivElement): void {
  act(() => {
    root.unmount();
  });
  container.remove();
}

async function flushEffects() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("useTeamCreateModalLifecycleEffects", () => {
  const mockedApi = vi.mocked(api);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("uses fallback root when token is empty", async () => {
    const setForgeDefaultWorktreeRoot = vi.fn();
    const props: HookProps = {
      token: "",
      busy: null,
      showCreateTeamModal: false,
      leaderMemberId: "",
      teamForgeAgents: [],
      setForgeDefaultWorktreeRoot,
      setLeaderMemberId: vi.fn(),
      setShowCreateTeamModal: vi.fn(),
      setCreateTeamStage: vi.fn(),
    };

    const { root, container } = await mountHarness(props);
    try {
      expect(mockedApi.getRuntimeDefaults).not.toHaveBeenCalled();
      expect(setForgeDefaultWorktreeRoot).toHaveBeenCalledWith(DEFAULT_WORKTREE_ROOT);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("loads and normalizes runtime default worktree root when token exists", async () => {
    mockedApi.getRuntimeDefaults.mockResolvedValueOnce({
      default_worktree_root: " /tmp/team-worktrees/ ",
    } as Awaited<ReturnType<typeof api.getRuntimeDefaults>>);

    const setForgeDefaultWorktreeRoot = vi.fn();
    const props: HookProps = {
      token: "token-123",
      busy: null,
      showCreateTeamModal: false,
      leaderMemberId: "",
      teamForgeAgents: [],
      setForgeDefaultWorktreeRoot,
      setLeaderMemberId: vi.fn(),
      setShowCreateTeamModal: vi.fn(),
      setCreateTeamStage: vi.fn(),
    };

    const { root, container } = await mountHarness(props);
    try {
      await flushEffects();
      expect(mockedApi.getRuntimeDefaults).toHaveBeenCalledWith("token-123");
      expect(setForgeDefaultWorktreeRoot).toHaveBeenCalledWith("/tmp/team-worktrees");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("falls back leader member id to the first forge agent when modal is open", async () => {
    const setLeaderMemberId = vi.fn();
    const props: HookProps = {
      token: "",
      busy: null,
      showCreateTeamModal: true,
      leaderMemberId: "",
      teamForgeAgents: [makeAgent("leader-a"), makeAgent("worker-b")],
      setForgeDefaultWorktreeRoot: vi.fn(),
      setLeaderMemberId,
      setShowCreateTeamModal: vi.fn(),
      setCreateTeamStage: vi.fn(),
    };

    const { root, container } = await mountHarness(props);
    try {
      expect(setLeaderMemberId).toHaveBeenCalledWith("leader-a");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("closes create modal on Escape when not busy", async () => {
    const setShowCreateTeamModal = vi.fn();
    const setCreateTeamStage = vi.fn();
    const props: HookProps = {
      token: "",
      busy: null,
      showCreateTeamModal: true,
      leaderMemberId: "",
      teamForgeAgents: [],
      setForgeDefaultWorktreeRoot: vi.fn(),
      setLeaderMemberId: vi.fn(),
      setShowCreateTeamModal,
      setCreateTeamStage,
    };

    const { root, container } = await mountHarness(props);
    try {
      await act(async () => {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
        await Promise.resolve();
      });
      expect(setShowCreateTeamModal).toHaveBeenCalledWith(false);
      expect(setCreateTeamStage).toHaveBeenCalledWith(0);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("ignores Escape while create-team action is busy", async () => {
    const setShowCreateTeamModal = vi.fn();
    const setCreateTeamStage = vi.fn();
    const props: HookProps = {
      token: "",
      busy: "create-team",
      showCreateTeamModal: true,
      leaderMemberId: "",
      teamForgeAgents: [],
      setForgeDefaultWorktreeRoot: vi.fn(),
      setLeaderMemberId: vi.fn(),
      setShowCreateTeamModal,
      setCreateTeamStage,
    };

    const { root, container } = await mountHarness(props);
    try {
      await act(async () => {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
        await Promise.resolve();
      });
      expect(setShowCreateTeamModal).not.toHaveBeenCalled();
      expect(setCreateTeamStage).not.toHaveBeenCalled();
    } finally {
      cleanupHarness(root, container);
    }
  });
});
