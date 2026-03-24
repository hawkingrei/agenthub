import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { AgentRecord } from "../../api";
import { api } from "../../api";

type UseTeamMemberBackfillEffectParams = {
  token: string;
  agents: AgentRecord[];
  teamSpecMemberIds: string[];
  teamMemberAgentsById: Record<string, AgentRecord | null>;
  setTeamMemberAgentsById: Dispatch<
    SetStateAction<Record<string, AgentRecord | null>>
  >;
};

function areAgentRecordsEqual(
  left: AgentRecord | null | undefined,
  right: AgentRecord | null
): boolean {
  if (right === null) {
    return left === null;
  }
  if (left == null) {
    return false;
  }
  if (left.args.length !== right.args.length) {
    return false;
  }
  for (let index = 0; index < left.args.length; index += 1) {
    if (left.args[index] !== right.args[index]) {
      return false;
    }
  }
  return (
    left.id === right.id &&
    left.name === right.name &&
    left.workdir === right.workdir &&
    left.command === right.command &&
    left.target_node_id === right.target_node_id &&
    left.worktree_mode === right.worktree_mode &&
    left.worktree_repo === right.worktree_repo &&
    left.worktree_ref === right.worktree_ref &&
    left.code_mode === right.code_mode &&
    left.agent_loop_enabled === right.agent_loop_enabled &&
    left.agent_loop_idle_seconds === right.agent_loop_idle_seconds &&
    left.agent_loop_prompt === right.agent_loop_prompt &&
    left.status === right.status &&
    left.created_at === right.created_at &&
    left.updated_at === right.updated_at
  );
}

export function useTeamMemberBackfillEffect({
  token,
  agents,
  teamSpecMemberIds,
  teamMemberAgentsById,
  setTeamMemberAgentsById,
}: UseTeamMemberBackfillEffectParams) {
  useEffect(() => {
    if (!token || teamSpecMemberIds.length === 0) {
      return;
    }
    const listedAgentIds = new Set(agents.map((agent) => agent.id));
    const unresolvedMemberIds = teamSpecMemberIds.filter((memberId) => {
      if (listedAgentIds.has(memberId)) {
        return false;
      }
      return teamMemberAgentsById[memberId] !== null;
    });
    if (unresolvedMemberIds.length === 0) {
      return;
    }

    let canceled = false;
    const loadMissingMemberAgents = async () => {
      const resolved = await Promise.all(
        unresolvedMemberIds.map(async (memberId) => {
          try {
            const agent = await api.getAgent(token, memberId);
            return [memberId, agent] as const;
          } catch {
            return [memberId, null] as const;
          }
        })
      );
      if (canceled) {
        return;
      }
      setTeamMemberAgentsById((prev) => {
        const next = { ...prev };
        let changed = false;
        for (const [memberId, agent] of resolved) {
          if (areAgentRecordsEqual(prev[memberId], agent)) {
            continue;
          }
          next[memberId] = agent;
          changed = true;
        }
        return changed ? next : prev;
      });
    };

    void loadMissingMemberAgents();
    return () => {
      canceled = true;
    };
  }, [agents, teamMemberAgentsById, teamSpecMemberIds, token, setTeamMemberAgentsById]);
}
