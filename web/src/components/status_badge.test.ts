import { describe, expect, it } from "vitest";
import {
  resolveAgentStatusTone,
  resolveTeamLifecycleStatusTone,
  resolveTeamRunStatusTone,
} from "./status_badge";

describe("status badge tone mapping", () => {
  it("maps agent status to shared tones", () => {
    expect(resolveAgentStatusTone("running")).toBe("active");
    expect(resolveAgentStatusTone("idle")).toBe("warning");
    expect(resolveAgentStatusTone("failed")).toBe("danger");
    expect(resolveAgentStatusTone("completed")).toBe("inactive");
    expect(resolveAgentStatusTone("stopped")).toBe("inactive");
    expect(resolveAgentStatusTone("unknown")).toBe("neutral");
  });

  it("maps team lifecycle tones", () => {
    expect(resolveTeamLifecycleStatusTone("active")).toBe("active");
    expect(resolveTeamLifecycleStatusTone("inactive")).toBe("inactive");
    expect(resolveTeamLifecycleStatusTone("missing")).toBe("danger");
  });

  it("maps team run statuses", () => {
    expect(resolveTeamRunStatusTone("working")).toBe("active");
    expect(resolveTeamRunStatusTone("completed")).toBe("active");
    expect(resolveTeamRunStatusTone("submitted")).toBe("warning");
    expect(resolveTeamRunStatusTone("input_required")).toBe("warning");
    expect(resolveTeamRunStatusTone("failed")).toBe("danger");
    expect(resolveTeamRunStatusTone("canceled")).toBe("inactive");
    expect(resolveTeamRunStatusTone("idle")).toBe("inactive");
    expect(resolveTeamRunStatusTone("other")).toBe("neutral");
  });
});
