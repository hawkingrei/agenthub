import { useEffect } from "react";
import type { AgentRecord } from "../../api";
import { api } from "../../api";
import { normalizeRuntimeWorktreeRoot } from "../../worktree_defaults";
import type { CreateTeamStage } from "./state";

type UseTeamCreateModalEffectsParams = {
  token: string;
  defaultWorktreeRoot: string;
  showCreateTeamModal: boolean;
  coordinatorMemberId: string;
  teamForgeAgents: AgentRecord[];
  busy: string | null;
  setForgeDefaultWorktreeRoot: (value: string) => void;
  setCoordinatorMemberId: (value: string) => void;
  setShowCreateTeamModal: (value: boolean) => void;
  setCreateTeamStage: (value: CreateTeamStage) => void;
};

export function useTeamCreateModalEffects({
  token,
  defaultWorktreeRoot,
  showCreateTeamModal,
  coordinatorMemberId,
  teamForgeAgents,
  busy,
  setForgeDefaultWorktreeRoot,
  setCoordinatorMemberId,
  setShowCreateTeamModal,
  setCreateTeamStage,
}: UseTeamCreateModalEffectsParams) {
  useEffect(() => {
    if (!token) {
      setForgeDefaultWorktreeRoot(defaultWorktreeRoot);
      return;
    }
    api
      .getRuntimeDefaults(token)
      .then((defaults) => {
        const root = normalizeRuntimeWorktreeRoot(
          defaults.default_worktree_root,
          defaultWorktreeRoot
        );
        setForgeDefaultWorktreeRoot(root);
      })
      .catch(() => undefined);
  }, [defaultWorktreeRoot, setForgeDefaultWorktreeRoot, token]);

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
