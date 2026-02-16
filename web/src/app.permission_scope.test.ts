import { describe, expect, it } from "vitest";
import { AcpPermissionRecord } from "./api";
import { filterPermissionsForAgent } from "./app";

const buildPermission = (
  id: string,
  agentId: string,
  status = "pending"
): AcpPermissionRecord => ({
  id,
  agent_id: agentId,
  session_id: `${agentId}-session`,
  options: [],
  status,
  created_at: 1,
});

describe("filterPermissionsForAgent", () => {
  it("returns empty when active agent is null", () => {
    const input = [buildPermission("p1", "agent-a")];
    expect(filterPermissionsForAgent(input, null)).toEqual([]);
  });

  it("keeps only permission records that belong to active agent", () => {
    const input = [
      buildPermission("p1", "agent-a"),
      buildPermission("p2", "agent-b", "responded"),
      buildPermission("p3", "agent-a", "timeout"),
    ];
    expect(filterPermissionsForAgent(input, "agent-a").map((item) => item.id)).toEqual([
      "p1",
      "p3",
    ]);
  });
});
