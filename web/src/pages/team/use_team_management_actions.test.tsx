// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import { useTeamManagementActions } from "./use_team_management_actions";
import { loadTeamCreateDraft } from "./create_draft_storage";

vi.mock("../../api", async () => {
  const actual = await vi.importActual<typeof import("../../api")>("../../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      getTeamRuntime: vi.fn(),
      startTeam: vi.fn(),
      stopTeam: vi.fn(),
    },
  };
});

vi.mock("./create_draft_storage", () => ({
  loadTeamCreateDraft: vi.fn(),
  clearTeamCreateDraft: vi.fn(),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamManagementActions>[0];
type HookSnapshot = ReturnType<typeof useTeamManagementActions>;

function HookHarness({
  params,
  onCapture,
}: {
  params: HookParams;
  onCapture: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamManagementActions(params);
  useEffect(() => {
    onCapture(snapshot);
  }, [onCapture, snapshot]);
  return null;
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    busy: null,
    teams: [],
    runs: [],
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
            description: "Leader",
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
    selectedTeamId: "team-1",
    selectedTeamHasLeader: true,
    selectedTeamHasConfiguredMembers: true,
    teamExecutionBlockedReason: null,
    selectedTeamWorkerCount: 0,
    selectedTeamMemberStatuses: [],
    selectedAgentWorkspaceMemberId: "",
    selectedAgentWorkspaceAgent: null,
    selectedAgentLabel: "Agent",
    newTeamName: "Alpha Team",
    newTeamDescription: "team",
    teamMemberDraft: null,
    teamMemberEditDraft: null,
    teamMemberRoleOptions: [
      { value: "leader", label: "Leader", disabled: false },
      { value: "worker", label: "Worker", disabled: false },
    ],
    teamPromptDefaults: {
      leader_prompt: "lead",
      worker_prompt: "work",
    } as HookParams["teamPromptDefaults"],
    forgeDefaultWorktreeRoot: "/tmp/worktrees",
    forgeAgentName: "",
    forgeAgentWorkdir: "",
    forgeAgentPresetId: "codex",
    forgeAgentWorktreeMode: "create_worktree",
    forgeAgentWorktreeRepo: "",
    forgeAgentWorktreeRef: "",
    forgeAgentCodeMode: true,
    forgeAgentBusy: false,
    patchTeamCreate: vi.fn(),
    resetTeamDraft: vi.fn(),
    refreshTeams: vi.fn().mockResolvedValue(undefined),
    refreshAgents: vi.fn().mockResolvedValue(undefined),
    navigateToTeamDetail: vi.fn(),
    navigateToTeamSelector: vi.fn(),
    setError: vi.fn(),
    setWarning: vi.fn(),
    setBusy: vi.fn(),
    setAgents: vi.fn(),
    setTeams: vi.fn(),
    setSelectedTeamId: vi.fn(),
    setRuns: vi.fn(),
    setTeamRunBrowserByTeam: vi.fn(),
    setActiveRunId: vi.fn(),
    setRunLookupId: vi.fn(),
    setTeamSelectorFilter: vi.fn(),
    setTeamMemberDraft: vi.fn(),
    setTeamMemberEditDraft: vi.fn(),
    setShowTeamMemberEditModal: vi.fn(),
    setTeamRuntimeByTeamId: vi.fn(),
    setShowCreateTeamModal: vi.fn(),
    setShowForgeAgentForm: vi.fn(),
    setForgeAgentName: vi.fn(),
    setForgeAgentWorkdir: vi.fn(),
    setForgeAgentPresetId: vi.fn(),
    setForgeAgentWorktreeMode: vi.fn(),
    setForgeAgentWorktreeRepo: vi.fn(),
    setForgeAgentWorktreeRef: vi.fn(),
    setForgeAgentCodeMode: vi.fn(),
    setForgeAgentWorktreeError: vi.fn(),
    setForgeAgentBusy: vi.fn(),
    setTeamMemberAgentsById: vi.fn(),
    setMemberDiscoveryCardsById: vi.fn(),
    setMemberDiscoveryCardLoadingById: vi.fn(),
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

describe("useTeamManagementActions", () => {
  const mockedApi = vi.mocked(api);
  const mockedLoadTeamCreateDraft = vi.mocked(loadTeamCreateDraft);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("restores create-team draft into wizard state", async () => {
    mockedLoadTeamCreateDraft.mockReturnValueOnce({
      draft: {
        name: "Restored Team",
        description: "restored",
        leaderPrompt: "",
        workers: [
          {
            memberId: "worker-1",
            prompt: "",
          },
        ],
      },
      error: null,
    });
    const params = createParams();
    const mounted = await mountHook(params);
    try {
      act(() => {
        mounted.getSnapshot()?.openCreateTeamModal();
      });
      expect(params.resetTeamDraft).toHaveBeenCalled();
      expect(params.patchTeamCreate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Restored Team",
          description: "restored",
          showCreateTeamModal: true,
          showForgeAgentForm: false,
        })
      );
      expect(params.setShowCreateTeamModal).not.toHaveBeenCalled();
    } finally {
      mounted.cleanup();
    }
  });

  it("blocks starting team runtime when members are not configured", async () => {
    const params = createParams({
      selectedTeamHasConfiguredMembers: false,
      teamExecutionBlockedReason: "Add a leader first",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onStartTeamRuntime();
      });
      expect(params.setError).toHaveBeenCalledWith("Add a leader first");
      expect(mockedApi.startTeam).not.toHaveBeenCalled();
    } finally {
      mounted.cleanup();
    }
  });

  it("opens forge modal with leader-aware defaults and starts runtime optimistically", async () => {
    mockedApi.startTeam.mockResolvedValueOnce({
      status: "running",
      members: [{ action: "started" }],
    } as never);
    mockedApi.getTeamRuntime.mockResolvedValueOnce({
      team_id: "team-1",
      team_name: "Alpha Team",
      status: "running",
      members: [],
      created_at: 1,
      updated_at: 1,
    } as never);

    const params = createParams({
      selectedTeamHasLeader: false,
      selectedTeamMemberStatuses: [
        {
          member_id: "leader-1",
          role: "leader",
          status: "running",
          work_status: "idle",
        },
      ] as HookParams["selectedTeamMemberStatuses"],
    });
    const mounted = await mountHook(params);
    try {
      act(() => {
        mounted.getSnapshot()?.openTeamMemberForgeModal();
      });
      expect(params.setTeamMemberDraft).toHaveBeenCalled();
      expect(params.setShowForgeAgentForm).toHaveBeenCalledWith(true);
      expect(params.setForgeAgentCodeMode).toHaveBeenCalledWith(true);

      await act(async () => {
        await mounted.getSnapshot()?.onStartTeamRuntime();
      });

      expect(params.setBusy).toHaveBeenCalledWith("start-team");
      expect(mockedApi.startTeam).toHaveBeenCalledWith("token-1", "team-1");
      expect(params.setTeamRuntimeByTeamId).toHaveBeenCalled();
      expect(params.setWarning).toHaveBeenCalledWith("Team runtime updated (started=1)");
    } finally {
      mounted.cleanup();
    }
  });
});
