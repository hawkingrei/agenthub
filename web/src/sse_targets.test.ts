import { describe, expect, it } from "vitest";
import { AgentRecord } from "./api";
import { buildSseTargetAgentIds, encodeSseTargetAgentIds } from "./sse_targets";

function buildAgent(overrides: Partial<AgentRecord>): AgentRecord {
  return {
    id: "agent-1",
    name: "Agent",
    workdir: "/tmp",
    command: "codex",
    args: [],
    target_node_id: null,
    worktree_repo: null,
    worktree_ref: null,
    worktree_mode: "use_existing",
    code_mode: true,
    status: "running",
    created_at: 1,
    updated_at: 1,
    ...overrides,
  };
}

describe("sse target agent ids", () => {
  it("selects active agents and dedupes ids", () => {
    const ids = buildSseTargetAgentIds([
      buildAgent({ id: "agent-a", status: "running" }),
      buildAgent({ id: "agent-b", status: "idle" }),
      buildAgent({ id: "agent-a", status: "running" }),
      buildAgent({ id: "agent-c", status: "failed" }),
      buildAgent({ id: "agent-remote", status: "running", target_node_id: "node-east" }),
    ]);
    expect(ids).toEqual(["agent-a", "agent-b"]);
  });

  it("encodes comma-separated query ids", () => {
    const query = encodeSseTargetAgentIds([" agent-a ", "", "agent-b"]);
    expect(query).toBe("agent-a,agent-b");
  });
});
