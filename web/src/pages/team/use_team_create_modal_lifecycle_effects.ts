import { useEffect } from "react";
import { type AgentRecord, api } from "../../api";
import { normalizeRuntimeWorktreeRoot } from "../../worktree_defaults";
import { DEFAULT_WORKTREE_ROOT, type CreateTeamStage } from "./state";

type UseTeamCreateModalLifecycleEffectsOptions = {
  token: string;
  busy: string | null;
  showCreateTeamModal: boolean;
  coordinatorMemberId: string;
  teamForgeAgents: AgentRecord[];
  parseError: (err: unknown) => string;
  setError: (next: string | null) => void;
  setForgeDefaultWorktreeRoot: (next: string) => void;
  setCoordinatorMemberId: (next: string) => void;
  setShowCreateTeamModal: (next: boolean) => void;
  setCreateTeamStage: (next: CreateTeamStage) => void;
};

export function useTeamCreateModalLifecycleEffects(
  options: UseTeamCreateModalLifecycleEffectsOptions
) {
  const {
    token,
    busy,
    showCreateTeamModal,
    coordinatorMemberId,
    teamForgeAgents,
    parseError,
    setError,
    setForgeDefaultWorktreeRoot,
    setCoordinatorMemberId,
    setShowCreateTeamModal,
    setCreateTeamStage,
  } = options;

  useEffect(() => {
    if (!token) {
      setForgeDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      return;
    }
    api
      .getRuntimeDefaults(token)
      .then((defaults) => {
        const root = normalizeRuntimeWorktreeRoot(
          defaults.default_worktree_root,
          DEFAULT_WORKTREE_ROOT
        );
        setForgeDefaultWorktreeRoot(root);
      })
      .catch((err) => {
        if (!showCreateTeamModal) {
          return;
        }
        setError(`Failed to load Team Forge defaults: ${parseError(err)}`);
      });
  }, [parseError, setError, setForgeDefaultWorktreeRoot, showCreateTeamModal, token]);

  useEffect(() => {
    if (!showCreateTeamModal) return;
    if (coordinatorMemberId && teamForgeAgents.some((agent) => agent.id === coordinatorMemberId)) {
      return;
    }
    const fallbackCoordinatorId = teamForgeAgents[0]?.id ?? "";
    setCoordinatorMemberId(fallbackCoordinatorId);
  }, [coordinatorMemberId, setCoordinatorMemberId, showCreateTeamModal, teamForgeAgents]);

  useEffect(() => {
    if (!showCreateTeamModal) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (busy === "create-team") return;
      event.preventDefault();
      setShowCreateTeamModal(false);
      setCreateTeamStage(0);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [busy, setCreateTeamStage, setShowCreateTeamModal, showCreateTeamModal]);
}
