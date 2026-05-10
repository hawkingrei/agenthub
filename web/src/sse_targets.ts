import { AgentRecord } from "./api";

export function buildSseTargetAgentIds(agents: AgentRecord[]): string[] {
  const seen = new Set<string>();
  const ids: string[] = [];
  for (const agent of agents) {
    if (!(agent.status === "running" || agent.status === "starting")) continue;
    if (agent.target_node_id) continue;
    const id = agent.id.trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    ids.push(id);
  }
  return ids;
}

export function encodeSseTargetAgentIds(ids: string[]): string {
  return ids
    .map((id) => id.trim())
    .filter((id) => id.length > 0)
    .join(",");
}
