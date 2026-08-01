import { useCallback, type Dispatch, type SetStateAction } from "react";
import {
  AGENT_SOURCE_TEAM_FORGE,
  type AgentDiscoveryCardRecord,
  type AgentRecord,
  api,
  type TeamDefinitionRecord,
  type TeamPromptDefaultsRecord,
  type TeamRunRecord,
  type TeamRuntimeRecord,
} from "../../api";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentModelLabel,
  getAgentPreset,
  resolveAcpProviderForAgent,
  type AgentPresetId,
} from "../../agent_presets";
import {
  DEFAULT_CODEX_ACP_MODE,
  normalizeCodexAcpModeId,
} from "../../codex_acp_modes";
import { normalizeWorkdirInput } from "../../worktree_defaults";
import {
  appendTeamMemberToSpec,
  buildTeamMemberDraftFromSpec,
  buildEmptyTeamSpec,
  buildCoordinatorForgeDefaultWorkdir,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  updateTeamMemberProfileInSpec,
  type TeamMemberProfileDraft,
} from "./create_helpers";
import { clearTeamCreateDraft, loadTeamCreateDraft } from "./create_draft_storage";
import {
  resolveInitialTeamMemberRole,
  resolveCopiedTeamAgentName,
  resolveTeamForgeDefaults,
} from "./forge_helpers";
import type { TeamMemberAgentStatus } from "./member_helpers";
import { backfillEmptyWorkerDraftPrompts, resolveTeamPromptForRole } from "./member_helpers";
import { removeTeamMemberLookupEntry, updateCachedTeamRuntimeStatus } from "./page_helpers";
import {
  createInitialTeamCreateState,
  DEFAULT_WORKTREE_ROOT,
  type TeamCreateState,
} from "./state";
import type { TeamRunBrowserState } from "./state";

type UseTeamManagementActionsOptions = {
  token: string;
  busy: string | null;
  agents: AgentRecord[];
  teams: TeamDefinitionRecord[];
  runs: TeamRunRecord[];
  selectedTeam: TeamDefinitionRecord | null;
  selectedTeamId: string | null;
  selectedTeamHasCoordinator: boolean;
  selectedTeamHasConfiguredMembers: boolean;
  teamExecutionBlockedReason: string | null;
  selectedTeamWorkerCount: number;
  selectedTeamMemberStatuses: TeamMemberAgentStatus[];
  selectedMemberId: string;
  selectedAgentWorkspaceMemberId: string;
  selectedAgentWorkspaceAgent: AgentRecord | null;
  selectedAgentLabel: string;
  newTeamName: string;
  newTeamDescription: string;
  teamMemberDraft: TeamMemberProfileDraft | null;
  teamMemberEditDraft: TeamMemberProfileDraft | null;
  teamPromptDefaults: TeamPromptDefaultsRecord;
  forgeDefaultWorktreeRoot: string;
  forgeAgentName: string;
  forgeAgentWorkdir: string;
  forgeAgentPresetId: AgentPresetId;
  forgeAgentCodexAcpDefaultMode: string;
  forgeAgentWorktreeMode: "use_existing" | "create_worktree" | "reuse_worktree";
  forgeAgentWorktreeRepo: string;
  forgeAgentWorktreeRef: string;
  forgeAgentCodeMode: boolean;
  forgeAgentBusy: boolean;
  patchTeamCreate: (patch: Partial<TeamCreateState>) => void;
  resetTeamDraft: () => void;
  refreshTeams: () => Promise<void>;
  refreshAgents: () => Promise<unknown>;
  navigateToTeamDetail: (teamId: string) => void;
  navigateToTeamSelector: () => void;
  setError: Dispatch<SetStateAction<string | null>>;
  setWarning: Dispatch<SetStateAction<string | null>>;
  setBusy: Dispatch<SetStateAction<string | null>>;
  setAgents: Dispatch<SetStateAction<AgentRecord[]>>;
  setTeams: Dispatch<SetStateAction<TeamDefinitionRecord[]>>;
  setSelectedTeamId: Dispatch<SetStateAction<string | null>>;
  setRuns: Dispatch<SetStateAction<TeamRunRecord[]>>;
  setTeamRunBrowserByTeam: Dispatch<SetStateAction<Record<string, TeamRunBrowserState>>>;
  setActiveRunId: Dispatch<SetStateAction<string | null>>;
  setRunLookupId: (next: string) => void;
  setTeamSelectorFilter: Dispatch<SetStateAction<string>>;
  setTeamMemberDraft: Dispatch<SetStateAction<TeamMemberProfileDraft | null>>;
  setTeamMemberEditDraft: Dispatch<SetStateAction<TeamMemberProfileDraft | null>>;
  setShowTeamMemberEditModal: Dispatch<SetStateAction<boolean>>;
  setTeamRuntimeByTeamId: Dispatch<SetStateAction<Record<string, TeamRuntimeRecord>>>;
  setShowCreateTeamModal: (next: boolean) => void;
  setShowForgeAgentForm: (next: boolean) => void;
  setShowCopyExistingAgentModal: (next: boolean) => void;
  setForgeAgentName: (next: string) => void;
  setForgeAgentWorkdir: (next: string | ((prev: string) => string)) => void;
  setForgeAgentPresetId: (next: AgentPresetId) => void;
  setForgeAgentCodexAcpDefaultMode: (next: string) => void;
  setForgeAgentWorktreeMode: (next: "use_existing" | "create_worktree" | "reuse_worktree") => void;
  setForgeAgentWorktreeRepo: (next: string) => void;
  setForgeAgentWorktreeRef: (next: string) => void;
  setForgeAgentCodeMode: (next: boolean) => void;
  setForgeAgentWorktreeError: (next: string | null) => void;
  setForgeAgentBusy: (next: boolean) => void;
  setTeamMemberAgentsById: Dispatch<SetStateAction<Record<string, AgentRecord | null>>>;
  setMemberDiscoveryCardsById: Dispatch<
    SetStateAction<Record<string, AgentDiscoveryCardRecord | null>>
  >;
  setMemberDiscoveryCardLoadingById: Dispatch<SetStateAction<Record<string, boolean>>>;
};

export function useTeamManagementActions(options: UseTeamManagementActionsOptions) {
  const {
    token,
    busy,
    agents,
    teams,
    runs,
    selectedTeam,
    selectedTeamId,
    selectedTeamHasCoordinator,
    selectedTeamHasConfiguredMembers,
    teamExecutionBlockedReason,
    selectedTeamWorkerCount,
    selectedTeamMemberStatuses,
    selectedMemberId,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceAgent,
    selectedAgentLabel,
    newTeamName,
    newTeamDescription,
    teamMemberDraft,
    teamMemberEditDraft,
    teamPromptDefaults,
    forgeDefaultWorktreeRoot,
    forgeAgentName,
    forgeAgentWorkdir,
    forgeAgentPresetId,
    forgeAgentCodexAcpDefaultMode,
    forgeAgentWorktreeMode,
    forgeAgentWorktreeRepo,
    forgeAgentWorktreeRef,
    forgeAgentCodeMode,
    forgeAgentBusy,
    patchTeamCreate,
    resetTeamDraft,
    refreshTeams,
    refreshAgents,
    navigateToTeamDetail,
    navigateToTeamSelector,
    setError,
    setWarning,
    setBusy,
    setAgents,
    setTeams,
    setSelectedTeamId,
    setRuns,
    setTeamRunBrowserByTeam,
    setActiveRunId,
    setRunLookupId,
    setTeamSelectorFilter,
    setTeamMemberDraft,
    setTeamMemberEditDraft,
    setShowTeamMemberEditModal,
    setTeamRuntimeByTeamId,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setShowCopyExistingAgentModal,
    setForgeAgentName,
    setForgeAgentWorkdir,
    setForgeAgentPresetId,
    setForgeAgentCodexAcpDefaultMode,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRepo,
    setForgeAgentWorktreeRef,
    setForgeAgentCodeMode,
    setForgeAgentWorktreeError,
    setForgeAgentBusy,
    setTeamMemberAgentsById,
    setMemberDiscoveryCardsById,
    setMemberDiscoveryCardLoadingById,
  } = options;

  const refreshTeamRuntime = useCallback(
    async (teamId: string, options?: { apply?: boolean }) => {
      const runtime = await api.getTeamRuntime(token, teamId);
      if (options?.apply !== false) {
        setTeamRuntimeByTeamId((prev) => ({ ...prev, [teamId]: runtime }));
      }
      return runtime;
    },
    [setTeamRuntimeByTeamId, token]
  );

  const refreshCatalogAfterRuntimeChange = useCallback(() => {
    void Promise.all([refreshTeams(), refreshAgents()]).catch((err) => {
      setWarning(parseErrorMessage(err));
    });
  }, [refreshAgents, refreshTeams, setWarning]);

  const applyOptimisticTeamRuntime = useCallback(
    (
      teamId: string,
      teamName: string,
      runtime: Awaited<ReturnType<typeof api.startTeam>>,
      memberStatuses: TeamMemberAgentStatus[]
    ) => {
      setTeamRuntimeByTeamId((prev) => {
        const previousRuntime = prev[teamId];
        const optimisticRuntime = updateCachedTeamRuntimeStatus(
          previousRuntime,
          teamId,
          teamName,
          runtime.status as TeamRuntimeRecord["status"],
          runtime.members,
          (sessionStatus) => {
            if (runtime.status !== "running") {
              return sessionStatus ?? undefined;
            }
            return "running";
          },
          memberStatuses
        );
        if (!optimisticRuntime) {
          return prev;
        }
        return { ...prev, [teamId]: optimisticRuntime };
      });
    },
    [setTeamRuntimeByTeamId]
  );

  const openCreateTeamModal = useCallback(() => {
    const { draft: restoredDraft, error: restoreError } = loadTeamCreateDraft("wizard");
    setError(null);
    setWarning(null);
    if (restoreError) {
      setError(restoreError);
    }
    resetTeamDraft();
    if (restoredDraft) {
      patchTeamCreate({
        ...restoredDraft,
        coordinatorPrompt: restoredDraft.coordinatorPrompt || teamPromptDefaults.coordinator_prompt,
        workers: backfillEmptyWorkerDraftPrompts(restoredDraft.workers ?? [], teamPromptDefaults),
        showCreateTeamModal: true,
        showForgeAgentForm: false,
        showCopyExistingAgentModal: false,
        forgeAgentWorktreeError: null,
        forgeAgentBusy: false,
      });
      return;
    }
    setShowCreateTeamModal(true);
    setShowForgeAgentForm(false);
    setShowCopyExistingAgentModal(false);
    setForgeAgentWorktreeError(null);
  }, [
    patchTeamCreate,
    resetTeamDraft,
    setError,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setShowCopyExistingAgentModal,
    setForgeAgentWorktreeError,
    setWarning,
    teamPromptDefaults,
  ]);

  const closeCreateTeamModal = useCallback(() => {
    if (busy === "create-team") {
      return;
    }
    setShowCreateTeamModal(false);
  }, [busy, setShowCreateTeamModal]);

  const openTeamMemberForgeModal = useCallback(() => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    const role = resolveInitialTeamMemberRole(selectedTeamHasCoordinator);
    const defaults = resolveTeamForgeDefaults({
      teamName: selectedTeam.name,
      teamSpec: selectedTeam.spec,
      role,
      workerCount: selectedTeamWorkerCount,
      defaultWorktreeRoot: forgeDefaultWorktreeRoot,
      agentPresetId: DEFAULT_AGENT_PRESET_ID,
      promptDefaults: teamPromptDefaults,
    });

    setError(null);
    setWarning(null);
    setShowCopyExistingAgentModal(false);
    setTeamMemberDraft(defaults.draft);
    setShowForgeAgentForm(true);
    setForgeAgentName(defaults.agentName);
    setForgeAgentWorktreeMode(defaults.worktreeMode);
    setForgeAgentWorktreeRepo(defaults.worktreeRepo);
    setForgeAgentWorktreeRef(defaults.worktreeRef);
    setForgeAgentPresetId(DEFAULT_AGENT_PRESET_ID);
    setForgeAgentCodexAcpDefaultMode(DEFAULT_CODEX_ACP_MODE);
    setForgeAgentCodeMode(true);
    setForgeAgentWorktreeError(null);
    setForgeAgentWorkdir(defaults.agentWorkdir);
  }, [
    forgeDefaultWorktreeRoot,
    selectedTeam,
    selectedTeamHasCoordinator,
    selectedTeamWorkerCount,
    setError,
    setWarning,
    setShowCopyExistingAgentModal,
    setTeamMemberDraft,
    setShowForgeAgentForm,
    setForgeAgentCodeMode,
    setForgeAgentName,
    setForgeAgentPresetId,
    setForgeAgentCodexAcpDefaultMode,
    setForgeAgentWorkdir,
    setForgeAgentWorktreeError,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRef,
    setForgeAgentWorktreeRepo,
    teamPromptDefaults,
  ]);

  const openCopyExistingAgentModal = useCallback(() => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    setError(null);
    setWarning(null);
    setShowForgeAgentForm(false);
    setShowCopyExistingAgentModal(true);
  }, [selectedTeam, setError, setShowCopyExistingAgentModal, setShowForgeAgentForm, setWarning]);

  const closeCopyExistingAgentModal = useCallback(() => {
    if (busy === "copy-team-agent") {
      return;
    }
    setShowCopyExistingAgentModal(false);
  }, [busy, setShowCopyExistingAgentModal]);

  const closeTeamMemberForgeModal = useCallback(() => {
    if (forgeAgentBusy) {
      return;
    }
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
    setTeamMemberDraft(null);
  }, [forgeAgentBusy, setShowForgeAgentForm, setForgeAgentWorktreeError, setTeamMemberDraft]);

  const openTeamMemberEditModal = useCallback(() => {
    const editMemberId = selectedAgentWorkspaceMemberId.trim() || selectedMemberId.trim();
    if (!selectedTeam || !editMemberId) {
      setError("Select an agent first");
      return;
    }
    const editAgent =
      editMemberId === selectedAgentWorkspaceMemberId ? selectedAgentWorkspaceAgent : null;
    const draft = buildTeamMemberDraftFromSpec(
      selectedTeam.spec,
      editMemberId,
      editAgent,
      teamPromptDefaults
    );
    if (!draft) {
      setError("Unable to load the selected agent profile");
      return;
    }
    setError(null);
    setWarning(null);
    setTeamMemberEditDraft(draft);
    setShowTeamMemberEditModal(true);
  }, [
    selectedMemberId,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceAgent,
    selectedTeam,
    setError,
    setWarning,
    setTeamMemberEditDraft,
    setShowTeamMemberEditModal,
    teamPromptDefaults,
  ]);

  const closeTeamMemberEditModal = useCallback(() => {
    if (busy === "save-team-member-profile") {
      return;
    }
    setShowTeamMemberEditModal(false);
    setTeamMemberEditDraft(null);
  }, [busy, setShowTeamMemberEditModal, setTeamMemberEditDraft]);

  const onCreateForgeAgent = useCallback(async () => {
    if (forgeAgentBusy) {
      return;
    }
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!teamMemberDraft) {
      setError("Open Add Agent first");
      return;
    }

    const isCoordinatorRole = teamMemberDraft.role === "coordinator";
    const effectiveWorktreeMode = isCoordinatorRole ? "use_existing" : forgeAgentWorktreeMode;
    const effectiveWorktreeRepo = isCoordinatorRole ? "" : forgeAgentWorktreeRepo.trim();
    const effectiveWorktreeRef = isCoordinatorRole ? "" : forgeAgentWorktreeRef.trim();
    const normalizedRoot =
      normalizeWorkdirInput(forgeDefaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;
    const name = forgeAgentName.trim() || "agent";
    const workdirInput = normalizeWorkdirInput(forgeAgentWorkdir);
    const workdir =
      isCoordinatorRole && !workdirInput
        ? buildCoordinatorForgeDefaultWorkdir(normalizedRoot, name)
        : workdirInput;
    const workdirPayload =
      effectiveWorktreeMode === "create_worktree" &&
      normalizedRoot &&
      workdir === normalizedRoot
        ? ""
        : workdir;

    if (!workdirPayload && effectiveWorktreeMode !== "create_worktree") {
      setError("Agent workdir is required");
      return;
    }
    if (effectiveWorktreeMode !== "use_existing" && !effectiveWorktreeRepo) {
      setError("Worktree repo is required");
      return;
    }

    setForgeAgentBusy(true);
    setError(null);
    setForgeAgentWorktreeError(null);
    let createdAgentId: string | null = null;
    try {
      const preset = getAgentPreset(forgeAgentPresetId);
      const codexAcpDefaultMode =
        preset.provider === "codex"
          ? normalizeCodexAcpModeId(forgeAgentCodexAcpDefaultMode)
          : null;
      const supportsRuntimeProfile =
        preset.provider === "codex" || preset.provider === "claude";
      const created = await api.createAgent(token, {
        name,
        workdir: workdirPayload,
        command: preset.command,
        args: preset.args.slice(),
        source: AGENT_SOURCE_TEAM_FORGE,
        worktree_mode: effectiveWorktreeMode,
        worktree_repo: effectiveWorktreeRepo || null,
        worktree_ref: effectiveWorktreeRef || null,
        code_mode: forgeAgentCodeMode,
        codex_acp_default_mode: codexAcpDefaultMode,
        runtime_model: supportsRuntimeProfile
          ? teamMemberDraft.runtime_model.trim() || null
          : null,
        thinking_level: supportsRuntimeProfile
          ? teamMemberDraft.thinking_level.trim() || null
          : null,
      });
      createdAgentId = created.id;
      const nextSpec = appendTeamMemberToSpec(
        selectedTeam.spec,
        { ...teamMemberDraft, member_id: created.id },
        created,
        teamPromptDefaults
      );
      const updated = await api.updateTeamSpec(token, selectedTeam.id, {
        spec: nextSpec,
        expected_updated_at: selectedTeam.updated_at,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      setTeams((prev) =>
        [...prev.filter((team) => team.id !== updated.id), updated].sort((left, right) =>
          left.name.localeCompare(right.name)
        )
      );
      setSelectedTeamId(updated.id);
      setShowForgeAgentForm(false);
      setForgeAgentWorktreeError(null);
      setTeamMemberDraft(null);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      const status = (err as { status?: number })?.status ?? null;
      if (createdAgentId && status === 409) {
        try {
          await api.deleteAgent(token, createdAgentId);
        } catch {
          // Best-effort cleanup only. Ambiguous failures should not mask the original conflict.
        }
      }
      const hint = formatTeamForgeWorktreeError(err);
      setForgeAgentWorktreeError(hint);
      setError(hint ?? parseErrorMessage(err));
    } finally {
      setForgeAgentBusy(false);
    }
  }, [
    forgeAgentBusy,
    selectedTeam,
    teamMemberDraft,
    forgeAgentWorktreeMode,
    forgeAgentWorktreeRepo,
    forgeAgentWorktreeRef,
    forgeDefaultWorktreeRoot,
    forgeAgentName,
    forgeAgentWorkdir,
    forgeAgentPresetId,
    forgeAgentCodexAcpDefaultMode,
    forgeAgentCodeMode,
    setError,
    setForgeAgentBusy,
    setForgeAgentWorktreeError,
    token,
    teamPromptDefaults,
    setAgents,
    setTeams,
    setSelectedTeamId,
    setShowForgeAgentForm,
    setTeamMemberDraft,
    refreshTeamRuntime,
  ]);

  const onCopyExistingTeamAgent = useCallback(async (sourceAgentId: string) => {
    if (busy === "copy-team-agent") {
      return;
    }
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    const sourceAgent = agents.find((agent) => agent.id === sourceAgentId);
    if (!sourceAgent) {
      setError("Select an existing agent first");
      return;
    }

    const role = resolveInitialTeamMemberRole(selectedTeamHasCoordinator);
    const copiedAgentName = resolveCopiedTeamAgentName(
      sourceAgent.name,
      sourceAgent.id,
      role,
      selectedTeamWorkerCount
    );
    const copiedDraft: TeamMemberProfileDraft = {
      member_id: "",
      role,
      description: `Copied from existing agent ${sourceAgent.name || sourceAgent.id}.`,
      model: formatAgentModelLabel(sourceAgent.command, sourceAgent.args) || DEFAULT_AGENT_PRESET_ID,
      prompt: resolveTeamPromptForRole(teamPromptDefaults, role),
      skills: [],
      custom_skills: "",
      agent_loop_enabled: false,
      agent_loop_idle_seconds: "",
      agent_loop_prompt: "",
      codex_acp_default_mode:
        resolveAcpProviderForAgent(sourceAgent.command, sourceAgent.args) === "codex"
          ? normalizeCodexAcpModeId(sourceAgent.codex_acp_default_mode)
          : DEFAULT_CODEX_ACP_MODE,
      runtime_model: sourceAgent.runtime_model ?? "",
      thinking_level: sourceAgent.thinking_level ?? "",
    };

    const copiedWorktreeMode = role === "coordinator" ? "use_existing" : sourceAgent.worktree_mode;
    const copiedWorktreeRepo = role === "coordinator" ? null : sourceAgent.worktree_repo ?? null;
    const copiedWorktreeRef = role === "coordinator" ? null : sourceAgent.worktree_ref ?? null;
    const copiedCodexAcpDefaultMode =
      resolveAcpProviderForAgent(sourceAgent.command, sourceAgent.args) === "codex"
        ? normalizeCodexAcpModeId(sourceAgent.codex_acp_default_mode)
        : null;

    setBusy("copy-team-agent");
    setError(null);
    setWarning(null);
    let createdAgentId: string | null = null;
    try {
      const created = await api.createAgent(token, {
        name: copiedAgentName,
        workdir: sourceAgent.workdir,
        command: sourceAgent.command,
        args: sourceAgent.args.slice(),
        target_node_id: sourceAgent.target_node_id ?? null,
        source: AGENT_SOURCE_TEAM_FORGE,
        worktree_mode: copiedWorktreeMode,
        worktree_repo: copiedWorktreeRepo,
        worktree_ref: copiedWorktreeRef,
        code_mode: sourceAgent.code_mode,
        codex_acp_default_mode: copiedCodexAcpDefaultMode,
        ...(sourceAgent.runtime_model != null || sourceAgent.thinking_level != null
          ? {
              runtime_model: sourceAgent.runtime_model ?? null,
              thinking_level: sourceAgent.thinking_level ?? null,
            }
          : {}),
      });
      createdAgentId = created.id;
      const updated = await api.updateTeamSpec(token, selectedTeam.id, {
        spec: appendTeamMemberToSpec(
          selectedTeam.spec,
          { ...copiedDraft, member_id: created.id },
          created,
          teamPromptDefaults
        ),
        expected_updated_at: selectedTeam.updated_at,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      setTeams((prev) =>
        [...prev.filter((team) => team.id !== updated.id), updated].sort((left, right) =>
          left.name.localeCompare(right.name)
        )
      );
      setSelectedTeamId(updated.id);
      setShowCopyExistingAgentModal(false);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      if (createdAgentId && (err as { status?: number })?.status === 409) {
        void api.deleteAgent(token, createdAgentId).catch(() => undefined);
      }
      setError(formatTeamForgeWorktreeError(err) ?? parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    agents,
    busy,
    refreshTeamRuntime,
    selectedTeam,
    selectedTeamHasCoordinator,
    selectedTeamWorkerCount,
    setAgents,
    setBusy,
    setError,
    setSelectedTeamId,
    setShowCopyExistingAgentModal,
    setTeams,
    setWarning,
    teamPromptDefaults,
    token,
  ]);

  const onMoveExistingTeamAgent = useCallback(async (sourceAgentId: string) => {
    if (busy === "move-team-agent") return;
    if (!selectedTeam) return setError("Select a team first");
    const sourceAgent = agents.find((agent) => agent.id === sourceAgentId);
    if (!sourceAgent) return setError("Select an existing agent first");
    if (sourceAgent.status !== "created" && sourceAgent.status !== "stopped") {
      return setError("Stop the agent before moving it into a Team");
    }
    const role = resolveInitialTeamMemberRole(selectedTeamHasCoordinator);
    const draft: TeamMemberProfileDraft = {
      member_id: sourceAgent.id, role,
      description: "Moved from the global agent catalog.",
      model: formatAgentModelLabel(sourceAgent.command, sourceAgent.args) || DEFAULT_AGENT_PRESET_ID,
      prompt: resolveTeamPromptForRole(teamPromptDefaults, role),
      skills: [], custom_skills: "", agent_loop_enabled: false, agent_loop_idle_seconds: "", agent_loop_prompt: "",
      codex_acp_default_mode: DEFAULT_CODEX_ACP_MODE,
      runtime_model: sourceAgent.runtime_model ?? "", thinking_level: sourceAgent.thinking_level ?? "",
    };
    setBusy("move-team-agent");
    setError(null);
    try {
      const updated = await api.moveExistingAgentToTeam(token, selectedTeam.id, {
        agent_id: sourceAgent.id,
        spec: appendTeamMemberToSpec(selectedTeam.spec, draft, sourceAgent, teamPromptDefaults),
        expected_updated_at: selectedTeam.updated_at,
      });
      setAgents((prev) => prev.filter((agent) => agent.id !== sourceAgent.id));
      setTeams((prev) => [...prev.filter((team) => team.id !== updated.id), updated].sort((a, b) => a.name.localeCompare(b.name)));
      setSelectedTeamId(updated.id);
      setShowCopyExistingAgentModal(false);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [agents, busy, refreshTeamRuntime, selectedTeam, selectedTeamHasCoordinator, setAgents, setBusy, setError, setSelectedTeamId, setShowCopyExistingAgentModal, setTeams, teamPromptDefaults, token]);

  const onSaveTeamMemberProfile = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!teamMemberEditDraft) {
      setError("Open Edit Profile first");
      return;
    }
    setBusy("save-team-member-profile");
    setError(null);
    setWarning(null);
    try {
      const nextSpec = updateTeamMemberProfileInSpec(
        selectedTeam.spec,
        teamMemberEditDraft,
        teamPromptDefaults
      );
      const updated = await api.updateTeamSpec(token, selectedTeam.id, {
        spec: nextSpec,
        expected_updated_at: selectedTeam.updated_at,
      });
      const idleSeconds = teamMemberEditDraft.agent_loop_idle_seconds.trim();
      const parsedIdleSeconds = Number.parseInt(idleSeconds, 10);
      const loopPayload = {
        enabled: teamMemberEditDraft.agent_loop_enabled,
        idle_seconds:
          teamMemberEditDraft.agent_loop_enabled &&
          idleSeconds !== "" &&
          Number.isFinite(parsedIdleSeconds)
            ? parsedIdleSeconds
            : null,
        prompt:
          teamMemberEditDraft.agent_loop_enabled &&
          teamMemberEditDraft.agent_loop_prompt.trim()
            ? teamMemberEditDraft.agent_loop_prompt.trim()
            : null,
      };
      try {
        await api.setAgentLoop(token, teamMemberEditDraft.member_id, loopPayload);
        setAgents((prev) =>
          prev.map((agent) =>
            agent.id === teamMemberEditDraft.member_id
              ? {
                  ...agent,
                  agent_loop_enabled: loopPayload.enabled,
                  agent_loop_idle_seconds: loopPayload.idle_seconds,
                  agent_loop_prompt: loopPayload.prompt,
                }
              : agent
          )
        );
        setTeamMemberAgentsById((prev) => ({
          ...prev,
          [teamMemberEditDraft.member_id]: (() => {
            const existingAgent = prev[teamMemberEditDraft.member_id];
            if (!existingAgent) {
              return existingAgent;
            }
            return {
              ...existingAgent,
              agent_loop_enabled: loopPayload.enabled,
              agent_loop_idle_seconds: loopPayload.idle_seconds,
              agent_loop_prompt: loopPayload.prompt,
            } satisfies AgentRecord;
          })(),
        }));
      } catch (loopErr) {
        setWarning(`Agent loop settings were not applied: ${parseErrorMessage(loopErr)}`);
      }
      if (
        selectedAgentWorkspaceAgent &&
        resolveAcpProviderForAgent(
          selectedAgentWorkspaceAgent.command,
          selectedAgentWorkspaceAgent.args
        ) === "codex"
      ) {
        const codexMode = normalizeCodexAcpModeId(
          teamMemberEditDraft.codex_acp_default_mode
        );
        try {
          await api.setAgentCodexAcpDefaultMode(
            token,
            teamMemberEditDraft.member_id,
            codexMode
          );
          setAgents((prev) =>
            prev.map((agent) =>
              agent.id === teamMemberEditDraft.member_id
                ? { ...agent, codex_acp_default_mode: codexMode }
                : agent
            )
          );
          setTeamMemberAgentsById((prev) => ({
            ...prev,
            [teamMemberEditDraft.member_id]: (() => {
              const existingAgent = prev[teamMemberEditDraft.member_id];
              if (!existingAgent) {
                return existingAgent;
              }
              return {
                ...existingAgent,
                codex_acp_default_mode: codexMode,
              } satisfies AgentRecord;
            })(),
          }));
        } catch (codexModeErr) {
          setWarning(
            `Codex permission settings were not applied: ${parseErrorMessage(codexModeErr)}`
          );
        }
      }
      if (
        selectedAgentWorkspaceAgent &&
        ["codex", "claude"].includes(
          resolveAcpProviderForAgent(
            selectedAgentWorkspaceAgent.command,
            selectedAgentWorkspaceAgent.args
          ) ?? ""
        )
      ) {
        const runtimeProfile = {
          runtime_model: teamMemberEditDraft.runtime_model.trim() || null,
          thinking_level: teamMemberEditDraft.thinking_level.trim() || null,
        };
        try {
          await api.setAgentRuntimeProfile(
            token,
            teamMemberEditDraft.member_id,
            runtimeProfile
          );
          setAgents((prev) =>
            prev.map((agent) =>
              agent.id === teamMemberEditDraft.member_id
                ? { ...agent, ...runtimeProfile }
                : agent
            )
          );
          setTeamMemberAgentsById((prev) => {
            const existingAgent = prev[teamMemberEditDraft.member_id];
            return {
              ...prev,
              [teamMemberEditDraft.member_id]: existingAgent
                ? { ...existingAgent, ...runtimeProfile }
                : existingAgent,
            };
          });
        } catch (runtimeProfileErr) {
          setWarning(
            `Runtime profile settings were not applied: ${parseErrorMessage(runtimeProfileErr)}`
          );
        }
      }
      setTeams((prev) =>
        [...prev.filter((team) => team.id !== updated.id), updated].sort((left, right) =>
          left.name.localeCompare(right.name)
        )
      );
      setSelectedTeamId(updated.id);
      setShowTeamMemberEditModal(false);
      setTeamMemberEditDraft(null);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    selectedTeam,
    selectedAgentWorkspaceAgent,
    teamMemberEditDraft,
    setBusy,
    setError,
    setWarning,
    teamPromptDefaults,
    token,
    setAgents,
    setTeamMemberAgentsById,
    setTeams,
    setSelectedTeamId,
    setShowTeamMemberEditModal,
    setTeamMemberEditDraft,
    refreshTeamRuntime,
  ]);

  const onCreateTeam = useCallback(async () => {
    const name = newTeamName.trim();
    if (!name) {
      setError("Team name is required");
      return;
    }
    setBusy("create-team");
    setError(null);
    setWarning(null);
    try {
      const created = await api.createTeam(token, {
        name,
        description: newTeamDescription.trim() || undefined,
        spec: buildEmptyTeamSpec(),
      });
      const initial = createInitialTeamCreateState();
      setTeams((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      setSelectedTeamId(created.id);
      clearTeamCreateDraft();
      patchTeamCreate({
        ...initial,
        coordinatorPrompt: teamPromptDefaults.coordinator_prompt,
        showCreateTeamModal: false,
        forgeAgentPresetId: DEFAULT_AGENT_PRESET_ID,
        forgeAgentCodeMode: true,
      });
      setWarning("Team created. Add the first agent to make it the coordinator.");
      navigateToTeamDetail(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    newTeamDescription,
    newTeamName,
    navigateToTeamDetail,
    patchTeamCreate,
    setBusy,
    setError,
    setSelectedTeamId,
    setTeams,
    setWarning,
    teamPromptDefaults,
    token,
  ]);

  const onDeleteTeam = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    const confirmed = window.confirm(
      `Delete team "${selectedTeam.name}" and all associated runs/events/messages?`
    );
    if (!confirmed) {
      return;
    }

    setBusy("delete-team");
    setError(null);
    try {
      await api.deleteTeam(token, selectedTeam.id);

      const remainingTeams = teams.filter((team) => team.id !== selectedTeam.id);
      const remainingRuns = runs.filter((run) => run.team_id !== selectedTeam.id);

      setTeams(remainingTeams);
      setRuns(remainingRuns);
      setTeamRunBrowserByTeam((prev) => {
        const next = { ...prev };
        delete next[selectedTeam.id];
        return next;
      });
      setSelectedTeamId((current) => (current === selectedTeam.id ? null : current));
      setActiveRunId((current) =>
        current && remainingRuns.some((run) => run.id === current) ? current : null
      );
      setRunLookupId("");
      setTeamSelectorFilter("");
      navigateToTeamSelector();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    navigateToTeamSelector,
    runs,
    selectedTeam,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    setSelectedTeamId,
    setTeamRunBrowserByTeam,
    setTeams,
    setTeamSelectorFilter,
    teams,
    token,
  ]);

  const onStartSelectedTeamAgent = useCallback(async () => {
    if (!token || !selectedAgentWorkspaceAgent) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("start-team-member-agent");
    try {
      await api.startAgent(token, selectedAgentWorkspaceAgent.id);
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(`Started ${selectedAgentLabel}.`);
    } catch (err) {
      const message = parseErrorMessage(err);
      if (message.toLowerCase().includes("agent already running")) {
        void Promise.all([
          refreshAgents(),
          selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
        ]).catch(() => undefined);
        setWarning(`${selectedAgentLabel} is already running.`);
        return;
      }
      setError(message);
    } finally {
      setBusy(null);
    }
  }, [
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
    token,
  ]);

  const onStopSelectedTeamAgent = useCallback(async () => {
    if (!token || !selectedAgentWorkspaceAgent) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("stop-team-member-agent");
    try {
      await api.stopAgent(token, selectedAgentWorkspaceAgent.id);
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(`Stopped ${selectedAgentLabel}.`);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
    token,
  ]);

  const onDeleteSelectedTeamAgent = useCallback(async () => {
    if (!token || !selectedAgentWorkspaceAgent || !selectedAgentWorkspaceMemberId) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("delete-team-member-agent");
    try {
      await api.deleteAgent(token, selectedAgentWorkspaceAgent.id);
      setAgents((prev) => prev.filter((agent) => agent.id !== selectedAgentWorkspaceAgent.id));
      setTeamMemberAgentsById((prev) => ({
        ...prev,
        [selectedAgentWorkspaceMemberId]: null,
      }));
      setMemberDiscoveryCardsById((prev) =>
        removeTeamMemberLookupEntry(prev, selectedAgentWorkspaceMemberId)
      );
      setMemberDiscoveryCardLoadingById((prev) =>
        removeTeamMemberLookupEntry(prev, selectedAgentWorkspaceMemberId)
      );
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(
        `Deleted ${selectedAgentLabel}. The Team member remains in the spec until you edit the profile.`
      );
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedAgentWorkspaceMemberId,
    selectedTeamId,
    setAgents,
    setBusy,
    setError,
    setMemberDiscoveryCardLoadingById,
    setMemberDiscoveryCardsById,
    setTeamMemberAgentsById,
    setWarning,
    token,
  ]);

  const onForceNewTeamMemberSession = useCallback(async () => {
    if (!token || !selectedTeamId || !selectedAgentWorkspaceMemberId) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("force-new-session");
    try {
      const runtime = await api.forceTeamMemberNewSession(
        token,
        selectedTeamId,
        selectedAgentWorkspaceMemberId
      );
      void Promise.all([refreshTeamRuntime(selectedTeamId), refreshAgents()]).catch(
        () => undefined
      );
      setWarning(formatTeamRuntimeActionSummary("force", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentWorkspaceMemberId,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
    token,
  ]);

  const onStartTeamRuntime = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!selectedTeamHasConfiguredMembers) {
      setError(teamExecutionBlockedReason ?? "Add at least one agent first");
      return;
    }
    setBusy("start-team");
    setError(null);
    setWarning(null);
    try {
      const runtime = await api.startTeam(token, selectedTeam.id);
      applyOptimisticTeamRuntime(
        selectedTeam.id,
        selectedTeam.name,
        runtime,
        selectedTeamMemberStatuses
      );
      refreshCatalogAfterRuntimeChange();
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("start", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    applyOptimisticTeamRuntime,
    refreshTeamRuntime,
    refreshCatalogAfterRuntimeChange,
    selectedTeam,
    selectedTeamHasConfiguredMembers,
    selectedTeamMemberStatuses,
    teamExecutionBlockedReason,
    setBusy,
    setError,
    setWarning,
    token,
  ]);

  const onStopTeamRuntime = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    setBusy("stop-team");
    setError(null);
    setWarning(null);
    try {
      const runtime = await api.stopTeam(token, selectedTeam.id);
      applyOptimisticTeamRuntime(
        selectedTeam.id,
        selectedTeam.name,
        runtime,
        selectedTeamMemberStatuses
      );
      refreshCatalogAfterRuntimeChange();
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("stop", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    applyOptimisticTeamRuntime,
    refreshTeamRuntime,
    refreshCatalogAfterRuntimeChange,
    selectedTeam,
    selectedTeamMemberStatuses,
    setBusy,
    setError,
    setWarning,
    token,
  ]);

  return {
    openCreateTeamModal,
    closeCreateTeamModal,
    openTeamMemberForgeModal,
    openCopyExistingAgentModal,
    closeCopyExistingAgentModal,
    closeTeamMemberForgeModal,
    openTeamMemberEditModal,
    closeTeamMemberEditModal,
    refreshTeamRuntime,
    onCreateForgeAgent,
    onCopyExistingTeamAgent,
    onMoveExistingTeamAgent,
    onSaveTeamMemberProfile,
    onCreateTeam,
    onDeleteTeam,
    onStartSelectedTeamAgent,
    onStopSelectedTeamAgent,
    onDeleteSelectedTeamAgent,
    onForceNewTeamMemberSession,
    onStartTeamRuntime,
    onStopTeamRuntime,
  };
}

function formatTeamRuntimeActionSummary(
  action: "start" | "stop" | "force",
  members: ReadonlyArray<{ action: string }>
): string {
  const counts = members.reduce<Record<string, number>>((acc, member) => {
    acc[member.action] = (acc[member.action] ?? 0) + 1;
    return acc;
  }, {});
  const parts = Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`);
  const prefix =
    action === "start"
      ? "Team runtime updated"
      : action === "stop"
        ? "Team runtime stopped"
        : "Forced new session";
  return parts.length > 0 ? `${prefix} (${parts.join(", ")})` : prefix;
}
