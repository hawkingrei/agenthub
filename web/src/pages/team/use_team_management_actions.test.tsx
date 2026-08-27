// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type { TeamMemberAgentStatus } from "./member_helpers";
import { useTeamManagementActions } from "./use_team_management_actions";
import { loadTeamCreateDraft } from "./create_draft_storage";

vi.mock("../../api", async () => {
  const actual = await vi.importActual<typeof import("../../api")>("../../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      createTeam: vi.fn(),
      createAgent: vi.fn(),
      deleteAgent: vi.fn(),
      adoptExistingAgentToTeam: vi.fn(async (token, teamId, payload) => {
        const agent = await api.createAgent(token, {
          name: payload.name,
          workdir: "/repo/source",
          command: "agenthub-codex-acp",
          args: [],
          target_node_id: null,
          source: "team_forge",
          worktree_mode: "use_existing",
          worktree_repo: null,
          worktree_ref: null,
          code_mode: true,
          codex_acp_default_mode: "full-access",
        });
        const spec = JSON.parse(
          JSON.stringify(payload.spec)
            .split("__agenthub_adopted_member__")
            .join(agent.id)
        );
        const team = await api.updateTeamSpec(token, teamId, {
          spec,
          expected_updated_at: payload.expected_updated_at,
        });
        return { agent, team };
      }),
      setAgentCodexAcpDefaultMode: vi.fn(),
      setAgentRuntimeProfile: vi.fn(),
      setAgentLoop: vi.fn(),
      updateTeamSpec: vi.fn(),
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
    agents: [
      {
        id: "agent-source-1",
        name: "Source Agent",
        workdir: "/repo/source",
        command: "agenthub-codex-acp",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        status: "stopped",
        created_at: 1,
        updated_at: 1,
      },
    ],
    teams: [],
    runs: [],
    selectedTeam: {
      id: "team-1",
      name: "Alpha Team",
      description: "team",
      spec: {
        coordinator_member_id: "coordinator-1",
        members: [
          {
            member_id: "coordinator-1",
            role: "coordinator",
            description: "Coordinator",
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
    selectedTeamHasCoordinator: true,
    selectedTeamHasConfiguredMembers: true,
    teamExecutionBlockedReason: null,
    selectedTeamWorkerCount: 0,
    selectedTeamMemberStatuses: [],
    selectedMemberId: "",
    selectedAgentWorkspaceMemberId: "",
    selectedAgentWorkspaceAgent: null,
    selectedAgentLabel: "Agent",
    newTeamName: "Alpha Team",
    newTeamDescription: "team",
    teamMemberDraft: null,
    teamMemberEditDraft: null,
    teamPromptDefaults: {
      coordinator_prompt: "lead",
      worker_prompt: "work",
    } as HookParams["teamPromptDefaults"],
    forgeDefaultWorktreeRoot: "/tmp/worktrees",
    forgeAgentName: "",
    forgeAgentWorkdir: "",
    forgeAgentPresetId: "codex",
    forgeAgentCodexAcpDefaultMode: "full-access",
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
    setShowCopyExistingAgentModal: vi.fn(),
    setForgeAgentName: vi.fn(),
    setForgeAgentWorkdir: vi.fn(),
    setForgeAgentPresetId: vi.fn(),
    setForgeAgentCodexAcpDefaultMode: vi.fn(),
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
        newTeamName: "Restored Team",
        newTeamDescription: "restored",
        newTeamSpec: "{}",
        createTeamStage: 1,
        coordinatorMemberId: "",
        coordinatorModel: "",
        coordinatorPrompt: "",
        coordinatorSkills: [],
        coordinatorCustomSkills: "",
        workers: [
          {
            member_id: "worker-1",
            description: "",
            model: "",
            prompt: "",
            skills: [],
            custom_skills: "",
          },
        ],
        teamForgeAgentIds: [],
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
          newTeamName: "Restored Team",
          newTeamDescription: "restored",
          showCreateTeamModal: true,
          showForgeAgentForm: false,
          showCopyExistingAgentModal: false,
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
      teamExecutionBlockedReason: "Add a coordinator first",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onStartTeamRuntime();
      });
      expect(params.setError).toHaveBeenCalledWith("Add a coordinator first");
      expect(mockedApi.startTeam).not.toHaveBeenCalled();
    } finally {
      mounted.cleanup();
    }
  });

  it("opens the member edit modal from the selected overview member", async () => {
    const params = createParams({
      selectedMemberId: "coordinator-1",
      selectedAgentWorkspaceMemberId: "",
    });
    const mounted = await mountHook(params);
    try {
      act(() => {
        mounted.getSnapshot()?.openTeamMemberEditModal();
      });
      expect(params.setTeamMemberEditDraft).toHaveBeenCalledWith(
        expect.objectContaining({
          member_id: "coordinator-1",
          role: "coordinator",
          prompt: "lead",
        })
      );
      expect(params.setShowTeamMemberEditModal).toHaveBeenCalledWith(true);
      expect(params.setError).toHaveBeenCalledWith(null);
    } finally {
      mounted.cleanup();
    }
  });

  it("opens the member edit modal from the active agent workspace member", async () => {
    const params = createParams({
      selectedMemberId: "missing-member",
      selectedAgentWorkspaceMemberId: "coordinator-1",
      selectedAgentWorkspaceAgent: {
        id: "agent-source-1",
        name: "Coordinator Runtime",
        workdir: "/repo/source",
        command: "agenthub-codex-acp",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        status: "stopped",
        created_at: 1,
        updated_at: 2,
      },
    });
    const mounted = await mountHook(params);
    try {
      act(() => {
        mounted.getSnapshot()?.openTeamMemberEditModal();
      });
      expect(params.setTeamMemberEditDraft).toHaveBeenCalledWith(
        expect.objectContaining({
          member_id: "coordinator-1",
          role: "coordinator",
          prompt: "lead",
        })
      );
      expect(params.setShowTeamMemberEditModal).toHaveBeenCalledWith(true);
    } finally {
      mounted.cleanup();
    }
  });

  it("opens forge modal with coordinator-aware defaults and starts runtime optimistically", async () => {
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
      selectedTeamHasCoordinator: false,
      selectedTeamMemberStatuses: [
        {
          member_id: "coordinator-1",
          role: "coordinator",
          status: "running",
          missing_agent: false,
        },
      ] as TeamMemberAgentStatus[],
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

  it("creates a team shell without opening the first coordinator forge flow", async () => {
    mockedApi.createTeam.mockResolvedValueOnce({
      id: "team-2",
      name: "New Team",
      description: "mission",
      spec: {
        spec_version: 1,
        members: [],
      },
      created_at: 1,
      updated_at: 1,
    } as never);

    const params = createParams({
      newTeamName: "New Team",
      newTeamDescription: "mission",
      selectedTeam: null,
      selectedTeamId: null,
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCreateTeam();
      });

      expect(mockedApi.createTeam).toHaveBeenCalledWith("token-1", {
        name: "New Team",
        description: "mission",
        spec: { spec_version: 1, members: [] },
      });
      expect(params.setSelectedTeamId).toHaveBeenCalledWith("team-2");
      expect(params.patchTeamCreate).toHaveBeenCalledWith(
        expect.objectContaining({
          newTeamName: "",
          newTeamDescription: "",
          coordinatorPrompt: "lead",
          showCreateTeamModal: false,
          showForgeAgentForm: false,
          showCopyExistingAgentModal: false,
          forgeAgentName: "",
          forgeAgentPresetId: "codex",
          forgeAgentWorktreeMode: "use_existing",
          forgeAgentWorktreeRepo: "",
          forgeAgentWorktreeRef: "",
          forgeAgentCodeMode: true,
          forgeAgentWorktreeError: null,
        })
      );
      expect(params.setTeamMemberDraft).not.toHaveBeenCalled();
      expect(params.setWarning).toHaveBeenCalledWith(
        "Team created. Add the first agent to make it the coordinator."
      );
      expect(params.navigateToTeamDetail).toHaveBeenCalledWith("team-2");
    } finally {
      mounted.cleanup();
    }
  });

  it("best-effort deletes a forged agent when team spec update conflicts", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-forge-1",
      name: "agent",
      workdir: "/tmp/worktrees/agent",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "create_worktree",
      worktree_repo: "git@example.com/repo.git",
      worktree_ref: "main",
      code_mode: true,
      status: "running",
      created_at: 1,
      updated_at: 1,
    } as never);
    const conflict = new Error(JSON.stringify({ error: "team spec changed" })) as Error & {
      status?: number;
    };
    conflict.status = 409;
    mockedApi.updateTeamSpec.mockRejectedValueOnce(conflict);
    mockedApi.deleteAgent.mockResolvedValueOnce(undefined as never);

    const params = createParams({
      teamMemberDraft: {
        member_id: "",
        role: "worker",
        description: "Implementation specialist",
        model: "codex",
        prompt: "",
        skills: [],
        custom_skills: "",
        agent_loop_enabled: false,
        agent_loop_idle_seconds: "90",
        agent_loop_prompt: "",
        codex_acp_default_mode: "full-access",
        runtime_model: "",
        thinking_level: "",
      } satisfies TeamMemberProfileDraft,
      forgeAgentName: "Forge Worker",
      forgeAgentWorkdir: "/tmp/worktrees/forge-worker",
      forgeAgentPresetId: "codex",
      forgeAgentWorktreeMode: "create_worktree",
      forgeAgentWorktreeRepo: "git@example.com/repo.git",
      forgeAgentWorktreeRef: "main",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCreateForgeAgent();
      });
      expect(mockedApi.createAgent).toHaveBeenCalledWith(
        "token-1",
        expect.objectContaining({
          command: "agenthubd",
          args: ["acp", "codex"],
          codex_acp_default_mode: "full-access",
        })
      );
      expect(mockedApi.deleteAgent).toHaveBeenCalledWith("token-1", "agent-forge-1");
      expect(params.setError).toHaveBeenCalledWith("team spec changed");
    } finally {
      mounted.cleanup();
    }
  });

  it("persists edited team member codex startup mode for restart", async () => {
    mockedApi.updateTeamSpec.mockResolvedValueOnce({
      id: "team-1",
      name: "Alpha Team",
      description: "team",
      spec: {
        coordinator_member_id: "coordinator-1",
        members: [{ member_id: "worker-1", role: "worker" }],
        steps: [],
      },
      created_at: 1,
      updated_at: 2,
    } as never);
    mockedApi.setAgentLoop.mockResolvedValueOnce({ status: "ok" } as never);
    mockedApi.setAgentCodexAcpDefaultMode.mockResolvedValueOnce({ status: "ok" } as never);
    mockedApi.setAgentRuntimeProfile.mockResolvedValueOnce({ status: "ok" } as never);

    const params = createParams({
      selectedTeam: {
        id: "team-1",
        name: "Alpha Team",
        description: "team",
        spec: {
          coordinator_member_id: "coordinator-1",
          members: [
            { member_id: "coordinator-1", role: "coordinator" },
            { member_id: "worker-1", role: "worker", prompt: "work" },
          ],
          steps: [],
        },
        created_at: 1,
        updated_at: 1,
      } as HookParams["selectedTeam"],
      selectedAgentWorkspaceAgent: {
        id: "worker-1",
        name: "Worker One",
        workdir: "/repo/worker",
        command: "agenthub-codex-acp",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        codex_acp_default_mode: "auto",
        status: "stopped",
        created_at: 1,
        updated_at: 1,
      },
      teamMemberEditDraft: {
        member_id: "worker-1",
        role: "worker",
        description: "Worker",
        model: "codex",
        prompt: "work",
        skills: [],
        custom_skills: "",
        agent_loop_enabled: false,
        agent_loop_idle_seconds: "",
        agent_loop_prompt: "",
        codex_acp_default_mode: "yolo",
        runtime_model: "gpt-5",
        thinking_level: "high",
      },
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onSaveTeamMemberProfile();
      });

      expect(mockedApi.setAgentCodexAcpDefaultMode).toHaveBeenCalledWith(
        "token-1",
        "worker-1",
        "full-access"
      );
      expect(mockedApi.setAgentRuntimeProfile).toHaveBeenCalledWith("token-1", "worker-1", {
        runtime_model: "gpt-5",
        thinking_level: "high",
      });
      expect(mockedApi.updateTeamSpec).toHaveBeenCalledWith(
        "token-1",
        "team-1",
        expect.objectContaining({
          spec: expect.objectContaining({
            members: expect.arrayContaining([
              expect.objectContaining({
                member_id: "worker-1",
                runtime: expect.objectContaining({
                  codex_acp_default_mode: "full-access",
                  runtime_model: "gpt-5",
                  thinking_level: "high",
                }),
              }),
            ]),
          }),
        })
      );
    } finally {
      mounted.cleanup();
    }
  });

  it("copies an existing agent into the selected team as a new team-owned member", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-copy-1",
      name: "source-agent-worker-1",
      workdir: "/repo/source",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "stopped",
      created_at: 1,
      updated_at: 1,
    } as never);
    mockedApi.updateTeamSpec.mockResolvedValueOnce({
      id: "team-1",
      name: "Alpha Team",
      description: "team",
      spec: {
        coordinator_member_id: "coordinator-1",
        members: [
          { member_id: "coordinator-1", role: "coordinator" },
          { member_id: "agent-copy-1", role: "worker" },
        ],
        steps: [],
      },
      created_at: 1,
      updated_at: 2,
    } as never);
    mockedApi.getTeamRuntime.mockResolvedValueOnce({
      team_id: "team-1",
      team_name: "Alpha Team",
      status: "stopped",
      members: [],
      created_at: 1,
      updated_at: 2,
    } as never);

    const params = createParams();
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("agent-source-1");
      });

      expect(mockedApi.createAgent).toHaveBeenCalledWith("token-1", {
        name: "source-agent-worker-1",
        workdir: "/repo/source",
        command: "agenthub-codex-acp",
        args: [],
        target_node_id: null,
        source: "team_forge",
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        codex_acp_default_mode: "full-access",
      });
      expect(params.setBusy).toHaveBeenNthCalledWith(1, "copy-team-agent");
      expect(params.setBusy).toHaveBeenLastCalledWith(null);
      expect(params.setShowCopyExistingAgentModal).toHaveBeenCalledWith(false);
      expect(params.setSelectedTeamId).toHaveBeenCalledWith("team-1");
    } finally {
      mounted.cleanup();
    }
  });

  it("copies the first adopted agent as a coordinator with fresh workspace ownership", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-copy-2",
      name: "source-agent",
      workdir: "/repo/source",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "stopped",
      created_at: 1,
      updated_at: 1,
    } as never);
    mockedApi.updateTeamSpec.mockResolvedValueOnce({
      id: "team-2",
      name: "Coordinator Team",
      description: "team",
      spec: {
        coordinator_member_id: "agent-copy-2",
        members: [{ member_id: "agent-copy-2", role: "coordinator" }],
        steps: [],
      },
      created_at: 1,
      updated_at: 2,
    } as never);
    mockedApi.getTeamRuntime.mockResolvedValueOnce({
      team_id: "team-2",
      team_name: "Coordinator Team",
      status: "stopped",
      members: [],
      created_at: 1,
      updated_at: 2,
    } as never);

    const params = createParams({
      selectedTeamHasCoordinator: false,
      selectedTeamWorkerCount: 0,
      selectedTeam: {
        id: "team-2",
        name: "Coordinator Team",
        description: "team",
        spec: {
          spec_version: 1,
          members: [],
        },
        created_at: 1,
        updated_at: 1,
      } as HookParams["selectedTeam"],
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("agent-source-1");
      });

      expect(mockedApi.createAgent).toHaveBeenCalledWith("token-1", {
        name: "source-agent-coordinator",
        workdir: "/repo/source",
        command: "agenthub-codex-acp",
        args: [],
        target_node_id: null,
        source: "team_forge",
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        codex_acp_default_mode: "full-access",
      });
      expect(mockedApi.updateTeamSpec).toHaveBeenCalledWith(
        "token-1",
        "team-2",
        expect.objectContaining({
          spec: expect.objectContaining({
            coordinator_member_id: "agent-copy-2",
            members: [
              expect.objectContaining({
                member_id: "agent-copy-2",
                role: "coordinator",
                prompt: "lead",
              }),
            ],
          }),
        })
      );
    } finally {
      mounted.cleanup();
    }
  });

  it("surfaces adoption conflicts without client-side cleanup", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-copy-conflict",
      name: "source-agent-worker-1",
      workdir: "/repo/source",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "stopped",
      created_at: 1,
      updated_at: 1,
    } as never);
    mockedApi.updateTeamSpec.mockRejectedValueOnce(
      Object.assign(new Error("team spec changed"), { status: 409 })
    );
    mockedApi.deleteAgent.mockResolvedValueOnce(undefined as never);

    const params = createParams();
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("agent-source-1");
      });

      expect(mockedApi.deleteAgent).not.toHaveBeenCalled();
      expect(params.setError).toHaveBeenCalledWith("team spec changed");
      expect(params.setShowCopyExistingAgentModal).not.toHaveBeenCalledWith(false);
    } finally {
      mounted.cleanup();
    }
  });

  it("keeps the original adoption conflict error", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-copy-delete-fails",
      name: "source-agent-worker-1",
      workdir: "/repo/source",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "stopped",
      created_at: 1,
      updated_at: 1,
    } as never);
    mockedApi.updateTeamSpec.mockRejectedValueOnce(
      Object.assign(new Error("team spec changed"), { status: 409 })
    );

    const params = createParams();
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("agent-source-1");
      });

      expect(mockedApi.deleteAgent).not.toHaveBeenCalled();
      expect(params.setError).toHaveBeenCalledWith("team spec changed");
    } finally {
      mounted.cleanup();
    }
  });

  it("does not clean up locally for adoption failures", async () => {
    mockedApi.createAgent.mockResolvedValueOnce({
      id: "agent-copy-server-error",
      name: "source-agent-worker-1",
      workdir: "/repo/source",
      command: "agenthub-codex-acp",
      args: [],
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      status: "stopped",
      created_at: 1,
      updated_at: 1,
    } as never);
    mockedApi.updateTeamSpec.mockRejectedValueOnce(
      Object.assign(new Error("server error"), { status: 500 })
    );

    const params = createParams();
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("agent-source-1");
      });

      expect(mockedApi.deleteAgent).not.toHaveBeenCalled();
      expect(params.setError).toHaveBeenCalledWith("server error");
    } finally {
      mounted.cleanup();
    }
  });

  it("surfaces an error when the selected copy source is missing", async () => {
    const params = createParams({
      agents: [],
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onCopyExistingTeamAgent("missing-agent");
      });

      expect(params.setError).toHaveBeenCalledWith("Select an existing agent first");
      expect(mockedApi.createAgent).not.toHaveBeenCalled();
      expect(params.setBusy).not.toHaveBeenCalledWith("copy-team-agent");
    } finally {
      mounted.cleanup();
    }
  });

  it("deletes the selected Team agent only after the removal is confirmed", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    mockedApi.deleteAgent.mockResolvedValueOnce(undefined as never);
    const params = createParams({
      selectedAgentWorkspaceMemberId: "worker-1",
      selectedAgentWorkspaceAgent: {
        id: "worker-agent-1",
        name: "Worker One",
        workdir: "/repo/worker",
        command: "agenthub-codex-acp",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        status: "stopped",
        created_at: 1,
        updated_at: 1,
      },
      selectedAgentLabel: "Worker One",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onDeleteSelectedTeamAgent();
      });

      expect(confirmSpy).toHaveBeenCalledWith('Delete agent "Worker One"?');
      expect(mockedApi.deleteAgent).toHaveBeenCalledWith("token-1", "worker-agent-1");
    } finally {
      confirmSpy.mockRestore();
      mounted.cleanup();
    }
  });

  it("does not delete the selected Team agent when the confirmation is declined", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    const params = createParams({
      selectedAgentWorkspaceMemberId: "worker-1",
      selectedAgentWorkspaceAgent: {
        id: "worker-agent-1",
        name: "Worker One",
        workdir: "/repo/worker",
        command: "agenthub-codex-acp",
        args: [],
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: true,
        status: "stopped",
        created_at: 1,
        updated_at: 1,
      },
      selectedAgentLabel: "Worker One",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await mounted.getSnapshot()?.onDeleteSelectedTeamAgent();
      });

      expect(mockedApi.deleteAgent).not.toHaveBeenCalled();
    } finally {
      confirmSpy.mockRestore();
      mounted.cleanup();
    }
  });
});
