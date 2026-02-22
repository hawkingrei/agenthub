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

export function useTeamMemberBackfillEffect({
  token,
  agents,
  teamSpecMemberIds,
  teamMemberAgentsById,
  setTeamMemberAgentsById,
}: UseTeamMemberBackfillEffectParams) {
  useEffect(() => {
    const listedAgentIds = new Set(agents.map((agent) => agent.id));
    const unresolvedMemberIds = teamSpecMemberIds.filter(
      (memberId) =>
        !listedAgentIds.has(memberId) && !(memberId in teamMemberAgentsById)
    );
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
        for (const [memberId, agent] of resolved) {
          next[memberId] = agent;
        }
        return next;
      });
    };

    void loadMissingMemberAgents();
    return () => {
      canceled = true;
    };
  }, [agents, teamMemberAgentsById, teamSpecMemberIds, token, setTeamMemberAgentsById]);
}
