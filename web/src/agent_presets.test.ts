import { describe, expect, it } from "vitest";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentCommand,
  getAgentPreset,
  isAgentPresetId,
  listAgentPresets,
  resolveAcpProvider,
} from "./agent_presets";

describe("agent presets", () => {
  it("lists known presets", () => {
    const ids = listAgentPresets().map((preset) => preset.id);
    expect(ids).toEqual(["codex", "gemini", "kimi"]);
  });

  it("validates preset ids", () => {
    expect(isAgentPresetId("codex")).toBe(true);
    expect(isAgentPresetId("gemini")).toBe(true);
    expect(isAgentPresetId("kimi")).toBe(true);
    expect(isAgentPresetId("unknown")).toBe(false);
  });

  it("formats command summaries", () => {
    const codex = getAgentPreset(DEFAULT_AGENT_PRESET_ID);
    const gemini = getAgentPreset("gemini");
    expect(formatAgentCommand(codex)).toBe("agenthub-codex-acp");
    expect(formatAgentCommand(gemini)).toBe("gemini --experimental-acp");
  });

  it("resolves ACP provider from command", () => {
    expect(resolveAcpProvider("agenthub-codex-acp")).toBe("codex");
    expect(resolveAcpProvider("/usr/local/bin/gemini")).toBe("gemini");
    expect(resolveAcpProvider("kimi")).toBe("kimi");
    expect(resolveAcpProvider("unknown")).toBe(null);
  });
});
