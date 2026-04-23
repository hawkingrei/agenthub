// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useTeamWorkspaceViewModel } from "./use_team_workspace_view_model";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamWorkspaceViewModel>[0];
type HookSnapshot = ReturnType<typeof useTeamWorkspaceViewModel>;

function HookHarness({
  params,
  onCapture,
}: {
  params: HookParams;
  onCapture: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamWorkspaceViewModel(params);
  useEffect(() => {
    onCapture(snapshot);
  }, [onCapture, snapshot]);
  return null;
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeam: {
      id: "team-1",
      name: "Alpha Team",
      description: "team",
      spec: {
        leader_member_id: "leader-1",
        members: [
          {
            member_id: "leader-1",
            role: "leader",
            description: "Leader agent",
            model: "codex",
            prompt: "lead",
            profile: null,
            runtime: null,
          },
        ],
        steps: [],
      },
      created_at: 1,
      updated_at: 1,
    } as HookParams["selectedTeam"],
    routeWorkspaceLens: null,
    tab: "conversation",
    focusedAgentMemberId: "",
    selectedMemberId: "",
    selectedTeamId: "team-1",
    selectedTeamMemberLiveStates: [],
    selectedTeamMemberSummary: null,
    selectedTeamRuntimeStatus: { label: "team running", status: "running" },
    selectedAgentWorkspaceMemberId: "",
    selectedAgentWorkspaceLiveState: null,
    activeRunForSelectedTeam: {
      id: "run-1",
      team_id: "team-1",
      context_id: "ctx-1",
      status: "working",
      input: {},
      created_at: 1,
      started_at: null,
      ended_at: null,
    } as HookParams["activeRunForSelectedTeam"],
    activeRunIdForSelectedTeam: "run-1",
    selectedConversation: {
      id: "shared-thread",
      team_id: "team-1",
      title: "all",
      status: "working",
      kind: "shared_thread",
      summary: null,
      latest_message_at: 1,
      created_at: 1,
      updated_at: 1,
    } as HookParams["selectedConversation"],
    selectedChannelId: "all",
    selectedChannelLabel: "# all",
    selectedChannelDescription:
      "Shared coordination lane for requests, updates, and cross-cutting discussion.",
    runsLoading: false,
    isCompactWorkbench: false,
    teamPromptDefaults: {
      leader_prompt: "lead",
      worker_prompt: "work",
    } as HookParams["teamPromptDefaults"],
    teamMemberAgentsById: {},
    agents: [],
    setTab: vi.fn(),
    setFocusedAgentMemberId: vi.fn(),
    setSelectedConversationTaskId: vi.fn(),
    setSelectedMemberId: vi.fn(),
    setTeamsSidebarCollapsed: vi.fn(),
    setActiveRunId: vi.fn(),
    setRunLookupId: vi.fn(),
    navigateToTeamLens: vi.fn(),
    navigateToTeamDetail: vi.fn(),
    navigateToTeamMemberWorkspace: vi.fn(),
    navigateToSidebarTeam: vi.fn(),
    ...overrides,
  };
}

async function mountHook(params: HookParams) {
  let snapshot: HookSnapshot | null = null;
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(
      <HookHarness
        params={params}
        onCapture={(next) => {
          snapshot = next;
        }}
      />
    );
    await Promise.resolve();
  });
  return {
    getSnapshot: () => snapshot,
    cleanup: () => {
      act(() => {
        root.unmount();
      });
      container.remove();
    },
  };
}

describe("useTeamWorkspaceViewModel", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("derives channel-first workspace chrome for shared conversation", async () => {
    const params = createParams();
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.activeWorkspaceLens).toBe("channels");
      expect(snapshot?.workspaceLensItems.map((item) => item.label)).toEqual([
        "Channels",
        "Tasks",
        "Members",
        "Search",
      ]);
      expect(snapshot?.workspaceTitle).toBe("# all");
      expect(snapshot?.workspaceDescription).toBe(
        "Shared coordination lane for requests, updates, and cross-cutting discussion."
      );
      expect(snapshot?.showWorkspaceRuntimeBadge).toBe(false);
      expect(snapshot?.workspaceNoticeText).toBeNull();
    } finally {
      mounted.cleanup();
    }
  });

  it("routes agent workspace selections through team detail and compact sidebar collapse", async () => {
    const prefetchWorkspaceLens = vi.fn();
    const params = createParams({
      tab: "agent_acp",
      focusedAgentMemberId: "worker-1",
      selectedAgentWorkspaceMemberId: "worker-1",
      selectedMemberId: "worker-1",
      isCompactWorkbench: true,
      prefetchWorkspaceLens,
      selectedTeamMemberLiveStates: [
        {
          member_id: "worker-1",
          agent_name: "Worker One",
          lifecycle_status: "running",
          lifecycle_tone: "active",
          run_status: "working",
          step_status: "working",
          pending_inbox_count: 2,
          current_work: "Investigating issue",
          role: "worker",
        },
      ] as HookParams["selectedTeamMemberLiveStates"],
    });
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.isAgentWorkspace).toBe(true);
      expect(snapshot?.selectedAgentLabel).toBe("Worker One");
      expect(snapshot?.workspaceNoticeDotClassName).toContain("bg-emerald-500");

      act(() => {
        snapshot?.onSelectAgentWorkspace("worker-2", "mailbox");
      });

      expect(params.setSelectedMemberId).toHaveBeenCalledWith("worker-2");
      expect(params.setFocusedAgentMemberId).toHaveBeenCalledWith("worker-2");
      expect(params.setTab).toHaveBeenCalledWith("mailbox");
      expect(params.navigateToTeamMemberWorkspace).toHaveBeenCalledWith(
        "team-1",
        "worker-2",
        "mailbox"
      );
      expect(params.setTeamsSidebarCollapsed).toHaveBeenCalledWith(true);

      act(() => {
        snapshot?.onSelectWorkspaceLens("search");
      });
      expect(params.navigateToTeamLens).toHaveBeenCalledWith("team-1", "search");
      act(() => {
        snapshot?.workspaceLensItems[2]?.onPrefetch?.();
      });
      expect(prefetchWorkspaceLens).toHaveBeenCalledWith("members");
    } finally {
      mounted.cleanup();
    }
  });

  it("uses selected member identity for mailbox title when focused workspace is cleared", async () => {
    const params = createParams({
      tab: "mailbox",
      focusedAgentMemberId: "",
      selectedMemberId: "worker-1",
      selectedTeamMemberLiveStates: [
        {
          member_id: "worker-1",
          agent_name: "Worker Mailbox",
          lifecycle_status: "running",
          lifecycle_tone: "active",
          run_status: "working",
          step_status: "working",
          pending_inbox_count: 1,
          current_work: "Replying to leader",
          role: "worker",
        },
      ] as HookParams["selectedTeamMemberLiveStates"],
    });
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.isAgentWorkspace).toBe(false);
      expect(snapshot?.workspaceTitle).toBe("Worker Mailbox");
      expect(snapshot?.workspaceDescription).toBe(
        "Direct mailbox thread for the selected member."
      );
    } finally {
      mounted.cleanup();
    }
  });

  it("uses canonical selected channel metadata for shared conversation and task chrome", async () => {
    const sharedConversationParams = createParams({
      selectedChannelLabel: "# review",
      selectedChannelDescription: "Focused design and implementation review lane.",
    });
    const sharedConversationMounted = await mountHook(sharedConversationParams);
    try {
      const snapshot = sharedConversationMounted.getSnapshot();
      expect(snapshot?.workspaceTitle).toBe("# review");
      expect(snapshot?.workspaceDescription).toBe(
        "Focused design and implementation review lane."
      );
    } finally {
      sharedConversationMounted.cleanup();
    }

    const tasksParams = createParams({
      tab: "tasks",
      selectedChannelLabel: "# review",
    });
    const tasksMounted = await mountHook(tasksParams);
    try {
      const snapshot = tasksMounted.getSnapshot();
      expect(snapshot?.workspaceTitle).toBe("Kanban");
      expect(snapshot?.workspaceDescription).toBe(
        "Canonical Kanban for leader-planned, system-managed Team tasks. Human task requests belong in # review."
      );
    } finally {
      tasksMounted.cleanup();
    }
  });

  it("routes mailbox member selections through member workspace URLs", async () => {
    const params = createParams({
      selectedTeamMemberLiveStates: [
        {
          member_id: "worker-1",
          agent_name: "Worker One",
          lifecycle_status: "running",
          lifecycle_tone: "active",
          run_status: "working",
          step_status: "working",
          pending_inbox_count: 1,
          current_work: "Replying",
          role: "worker",
        },
      ] as HookParams["selectedTeamMemberLiveStates"],
    });
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      act(() => {
        snapshot?.onOpenMailboxForMember("worker-1");
      });
      expect(params.setSelectedMemberId).toHaveBeenCalledWith("worker-1");
      expect(params.setFocusedAgentMemberId).toHaveBeenCalledWith("worker-1");
      expect(params.setTab).toHaveBeenCalledWith("mailbox");
      expect(params.navigateToTeamMemberWorkspace).toHaveBeenCalledWith(
        "team-1",
        "worker-1",
        "mailbox"
      );
    } finally {
      mounted.cleanup();
    }
  });

  it("falls back to agent registry names and default chrome when no team is selected", async () => {
    const params = createParams({
      selectedTeam: null,
      selectedTeamId: null,
      selectedConversation: null,
      activeRunForSelectedTeam: null,
      activeRunIdForSelectedTeam: null,
      focusedAgentMemberId: "worker-2",
      routeWorkspaceLens: "search",
      agents: [
        {
          id: "worker-2",
          name: "Worker Two",
          workdir: "/repo",
          command: "codex",
          args: [],
          worktree_mode: "create_worktree",
          worktree_repo: null,
          worktree_ref: null,
          code_mode: true,
          agent_loop_enabled: false,
          agent_loop_idle_seconds: 0,
          agent_loop_prompt: "",
          status: "running",
          created_at: 1,
          updated_at: 1,
        },
      ] as HookParams["agents"],
    });
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.activeWorkspaceLens).toBe("search");
      expect(snapshot?.selectedAgentLabel).toBe("Worker Two");
      expect(snapshot?.workspaceTitle).toBe("Team Workbench");
      expect(snapshot?.workspaceDescription).toBe(
        "Select a team from the left rail to start team conversations and supervise execution."
      );
      expect(snapshot?.showDedicatedWorkspaceHeading).toBe(true);
    } finally {
      mounted.cleanup();
    }
  });

  it("preserves the selected channel id when routing back into the channels lens", async () => {
    const params = createParams({
      selectedChannelId: "review",
      selectedChannelLabel: "# review",
      selectedChannelDescription: "Focused design and implementation review lane.",
    });
    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      act(() => {
        snapshot?.onSelectConversationSubject();
      });
      expect(params.navigateToTeamLens).toHaveBeenCalledWith("team-1", "channels", "review");

      act(() => {
        snapshot?.onSelectWorkspaceLens("channels");
      });
      expect(params.navigateToTeamLens).toHaveBeenLastCalledWith("team-1", "channels", "review");
    } finally {
      mounted.cleanup();
    }
  });
});
