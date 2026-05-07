import { describe, expect, it } from "vitest";
import { normalizeAcpModeId } from "./acp_mode";

describe("normalizeAcpModeId", () => {
  it("maps yolo-style aliases to the Codex full-access mode id", () => {
    expect(normalizeAcpModeId("yolo")).toBe("full-access");
    expect(normalizeAcpModeId("yalo")).toBe("full-access");
    expect(normalizeAcpModeId("danger_full_access")).toBe("full-access");
    expect(normalizeAcpModeId("danger-full-access")).toBe("full-access");
  });

  it("trims mode ids without changing canonical values", () => {
    expect(normalizeAcpModeId(" full-access ")).toBe("full-access");
    expect(normalizeAcpModeId("auto")).toBe("auto");
    expect(normalizeAcpModeId("read-only")).toBe("read-only");
  });
});
