import { useEffect } from "react";
import { type AgentRecord, api } from "../../api";
import { normalizeRuntimeWorktreeRoot } from "../../worktree_defaults";
import { DEFAULT_WORKTREE_ROOT, type CreateTeamStage } from "./state";

type UseTeamCreateModalLifecycleEffectsOptions = {
  token: string;
  busy: string | null;
  showCreateTeamModal: boolean;
  leaderMemberId: string;
  teamForgeAgents: AgentRecord[];
  parseError: (err: unknown) => string;
  setError: (next: string | null) => void;
  setForgeDefaultWorktreeRoot: (next: string) => void;
  setLeaderMemberId: (next: string) => void;
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
    leaderMemberId,
    teamForgeAgents,
    parseError,
    setError,
    setForgeDefaultWorktreeRoot,
    setLeaderMemberId,
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
    if (leaderMemberId && teamForgeAgents.some((agent) => agent.id === leaderMemberId)) {
      return;
    }
    const fallbackLeaderId = teamForgeAgents[0]?.id ?? "";
    setLeaderMemberId(fallbackLeaderId);
  }, [leaderMemberId, setLeaderMemberId, showCreateTeamModal, teamForgeAgents]);

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
