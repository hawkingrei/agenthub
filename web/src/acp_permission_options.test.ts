import { describe, expect, it } from "vitest";
import {
  isRejectPermissionOption,
  resolveAcpPermissionDecisionText,
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
    expect(isRejectPermissionOption(option("allow_once"))).toBe(false);
    expect(isRejectPermissionOption(option("reject_once"))).toBe(true);
    expect(isRejectPermissionOption(option("reject_always"))).toBe(true);
  });

  it("formats completed permission decisions", () => {
    const options = [
      { option_id: "allow", name: "Allow once", kind: "allow_once" },
      { option_id: "reject", name: "Reject", kind: "reject_once" },
    ];

    expect(resolveAcpPermissionDecisionText(options, null)).toBe("Denied");
    expect(resolveAcpPermissionDecisionText(options, "allow")).toBe("Approved · Allow");
    expect(resolveAcpPermissionDecisionText(options, "reject")).toBe("Denied · Deny");
    expect(resolveAcpPermissionDecisionText(options, "missing")).toBe("Approved");
  });
});
