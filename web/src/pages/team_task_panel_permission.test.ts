import { describe, expect, it } from "vitest";
import type { AcpPermissionRecord } from "../api";
import {
  type PermissionReviewCardPayload,
  resolvePermissionStatusText,
} from "./team_task_panel";

const payload: PermissionReviewCardPayload = {
  type: "permission_review_card",
  permission_id: "perm-1",
  agent_id: "agent-1",
  agent_session_id: "session-1",
  acp_session_id: "acp-session-1",
  tool_call_id: "call-1",
  tool_call: null,
  tool_name: "git push",
  requester_actor_id: null,
  requester_role: null,
  options: [
    { option_id: "allow", name: "Allow once", kind: "allow_once" },
    { option_id: "reject", name: "Reject", kind: "reject_once" },
  ],
  summary: "Run git push",
  reason: null,
  reason_text: null,
  status: "pending",
};

const record = (selectedOptionId: string | null): AcpPermissionRecord => ({
  id: "perm-1",
  agent_id: "agent-1",
  session_id: "session-1",
  options: payload.options,
  status: "responded",
  selected_option_id: selectedOptionId,
  created_at: 1,
  responded_at: 2,
});

describe("TeamTaskPanel permission status text", () => {
  it("treats no selected option as a denied permission", () => {
    expect(resolvePermissionStatusText({ ...payload, status: "responded" })).toBe("Denied");
  });

  it("labels selected reject options as denied decisions", () => {
    expect(resolvePermissionStatusText(payload, record("reject"))).toBe("Denied · Deny");
  });

  it("keeps unknown selected options as approved fallback", () => {
    expect(resolvePermissionStatusText(payload, record("missing"))).toBe("Approved");
  });
});
