import { describe, expect, it } from "vitest";
import { AcpPermissionRecord } from "./api";
import { buildPermissionCopyText, derivePermissionTitle } from "./components/acp_debug";

const basePermission: AcpPermissionRecord = {
  id: "perm-1",
  agent_id: "agent-1",
  session_id: "session-1",
  options: [],
  status: "pending",
  created_at: 1,
};

describe("acp debug permission helpers", () => {
  it("prefers tool call title over fallback IDs", () => {
    const title = derivePermissionTitle(
      { ...basePermission, tool_call_id: "call-1" },
      { title: "Run Tool Demo" }
    );
    expect(title).toBe("Run Tool Demo");
  });

  it("builds structured copy payload", () => {
    const text = buildPermissionCopyText({
      ...basePermission,
      status: "responded",
      selected_option_id: "allow_once",
      options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
      tool_call: { title: "Permission Demo", raw_input: { a: 1 } },
      responded_at: 2,
    });
    const payload = JSON.parse(text) as Record<string, unknown>;
    expect(payload.permission_id).toBe("perm-1");
    expect(payload.status).toBe("responded");
    expect(payload.selected_option_id).toBe("allow_once");
    expect(payload.tool_call).toEqual({ title: "Permission Demo", raw_input: { a: 1 } });
  });
});
