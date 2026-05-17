import { useCallback, useReducer, useState } from "react";
import {
  DEFAULT_TEAM_CREATE_STATE,
  DEFAULT_WORKTREE_ROOT,
  reduceTeamCreateState,
  resolveUpdater,
  type TeamCreateState,
} from "./state";
import type { AgentPresetId } from "../../agent_presets";
import type { TeamMemberProfileDraft } from "./create_helpers";
import {
  EMPTY_TEAM_PROMPT_DEFAULTS,
} from "./member_helpers";

export function useTeamCreateState() {
  const [state, dispatch] = useReducer(reduceTeamCreateState, undefined, () => ({
    ...DEFAULT_TEAM_CREATE_STATE,
  }));

  const [forgeDefaultWorktreeRoot, setForgeDefaultWorktreeRoot] = useState(
    DEFAULT_WORKTREE_ROOT
  );
  const [teamPromptDefaults, setTeamPromptDefaults] = useState(EMPTY_TEAM_PROMPT_DEFAULTS);
  const [teamMemberDraft, setTeamMemberDraft] = useState<TeamMemberProfileDraft | null>(null);
  const [teamMemberEditDraft, setTeamMemberEditDraft] =
    useState<TeamMemberProfileDraft | null>(null);
  const [showTeamMemberEditModal, setShowTeamMemberEditModal] = useState(false);

  const patchTeamCreate = useCallback(
    (patch: Partial<TeamCreateState>) => {
      dispatch({ type: "patch", patch });
    },
    [dispatch]
  );

  const setNewTeamName = useCallback(
    (next: string) => patchTeamCreate({ newTeamName: next }),
    [patchTeamCreate]
  );

  const setNewTeamDescription = useCallback(
    (next: string) => patchTeamCreate({ newTeamDescription: next }),
    [patchTeamCreate]
  );

  const setShowCreateTeamModal = useCallback(
    (next: boolean) => patchTeamCreate({ showCreateTeamModal: next }),
    [patchTeamCreate]
  );

  const setShowForgeAgentForm = useCallback(
    (next: boolean) => patchTeamCreate({ showForgeAgentForm: next }),
    [patchTeamCreate]
  );

  const setShowCopyExistingAgentModal = useCallback(
    (next: boolean) => patchTeamCreate({ showCopyExistingAgentModal: next }),
    [patchTeamCreate]
  );

  const setForgeAgentName = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentName: next }),
    [patchTeamCreate]
  );

  const setForgeAgentWorkdir = useCallback(
    (next: string | ((prev: string) => string)) =>
      patchTeamCreate({
        forgeAgentWorkdir: resolveUpdater(state.forgeAgentWorkdir, next),
      }),
    [state.forgeAgentWorkdir, patchTeamCreate]
  );

  const patchTeamMemberDraft = useCallback((patch: Partial<TeamMemberProfileDraft>) => {
    setTeamMemberDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  }, []);

  const patchTeamMemberEditDraft = useCallback((patch: Partial<TeamMemberProfileDraft>) => {
    setTeamMemberEditDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  }, []);

  const setForgeAgentPresetId = useCallback(
    (next: AgentPresetId) => {
      patchTeamCreate({ forgeAgentPresetId: next });
      patchTeamMemberDraft({ model: next });
    },
    [patchTeamCreate, patchTeamMemberDraft]
  );

  const setForgeAgentCodexAcpDefaultMode = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentCodexAcpDefaultMode: next }),
    [patchTeamCreate]
  );

  const setForgeAgentWorktreeMode = useCallback(
    (next: "use_existing" | "create_worktree" | "reuse_worktree") =>
      patchTeamCreate({ forgeAgentWorktreeMode: next }),
    [patchTeamCreate]
  );

  const setForgeAgentWorktreeRepo = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentWorktreeRepo: next }),
    [patchTeamCreate]
  );

  const setForgeAgentWorktreeRef = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentWorktreeRef: next }),
    [patchTeamCreate]
  );

  const setForgeAgentCodeMode = useCallback(
    (next: boolean) => patchTeamCreate({ forgeAgentCodeMode: next }),
    [patchTeamCreate]
  );

  const setForgeAgentWorktreeError = useCallback(
    (next: string | null) => patchTeamCreate({ forgeAgentWorktreeError: next }),
    [patchTeamCreate]
  );

  const setForgeAgentBusy = useCallback(
    (next: boolean) => patchTeamCreate({ forgeAgentBusy: next }),
    [patchTeamCreate]
  );

  return {
    ...state,
    teamCreateState: state,
    forgeDefaultWorktreeRoot,
    setForgeDefaultWorktreeRoot,
    teamPromptDefaults,
    setTeamPromptDefaults,
    teamMemberDraft,
    setTeamMemberDraft,
    teamMemberEditDraft,
    setTeamMemberEditDraft,
    showTeamMemberEditModal,
    setShowTeamMemberEditModal,
    patchTeamCreate,
    setNewTeamName,
    setNewTeamDescription,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setShowCopyExistingAgentModal,
    setForgeAgentName,
    setForgeAgentWorkdir,
    patchTeamMemberDraft,
    patchTeamMemberEditDraft,
    setForgeAgentPresetId,
    setForgeAgentCodexAcpDefaultMode,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRepo,
    setForgeAgentWorktreeRef,
    setForgeAgentCodeMode,
    setForgeAgentWorktreeError,
    setForgeAgentBusy,
    dispatch,
  };
}
