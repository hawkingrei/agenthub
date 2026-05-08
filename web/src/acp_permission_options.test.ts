import { describe, expect, it } from "vitest";
import {
  hasRejectPermissionOption,
  resolveAcpPermissionOptionLabel,
} from "./acp_permission_options";
import type { AcpPermissionOption } from "./api";

const option = (kind: string, name = "Original label"): AcpPermissionOption => ({
  option_id: kind,
  name,
  kind,
});

describe("ACP permission option helpers", () => {
  it("maps ACP option kinds to Codex-aligned decision labels", () => {
    expect(resolveAcpPermissionOptionLabel(option("allow_once", "Allow once"))).toBe("Allow");
    expect(resolveAcpPermissionOptionLabel(option("allow_always", "Always allow"))).toBe(
      "Don't ask again"
    );
    expect(resolveAcpPermissionOptionLabel(option("reject_once", "Reject once"))).toBe("Deny");
    expect(resolveAcpPermissionOptionLabel(option("reject_always", "Reject always"))).toBe(
      "Deny and don't ask again"
    );
    expect(resolveAcpPermissionOptionLabel(option("custom", "Custom"))).toBe("Custom");
  });

  it("detects whether ACP already provided a reject decision", () => {
    expect(hasRejectPermissionOption([option("allow_once")])).toBe(false);
    expect(hasRejectPermissionOption([option("allow_once"), option("reject_once")])).toBe(true);
  });
});
