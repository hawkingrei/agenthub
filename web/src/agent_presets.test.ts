import { describe, expect, it } from "vitest";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentCommand,
  formatAgentModelLabel,
  getAgentPreset,
  isAgentPresetId,
  listAgentPresets,
  resolveAcpProvider,
} from "./agent_presets";

describe("agent presets", () => {
  it("lists known presets", () => {
    const ids = listAgentPresets().map((preset) => preset.id);
    expect(ids).toEqual([
      "codex",
      "gemini",
      "kimi",
      "claude_agent",
      "claude_code_rs",
    ]);
  });

  it("validates preset ids", () => {
    expect(isAgentPresetId("codex")).toBe(true);
    expect(isAgentPresetId("gemini")).toBe(true);
    expect(isAgentPresetId("kimi")).toBe(true);
    expect(isAgentPresetId("claude_agent")).toBe(true);
    expect(isAgentPresetId("claude_code_rs")).toBe(true);
    expect(isAgentPresetId("unknown")).toBe(false);
  });

  it("formats command summaries", () => {
    const codex = getAgentPreset(DEFAULT_AGENT_PRESET_ID);
    const gemini = getAgentPreset("gemini");
    const claudeAgent = getAgentPreset("claude_agent");
    const claudeCodeRs = getAgentPreset("claude_code_rs");
    expect(formatAgentCommand(codex)).toBe("agenthub-codex-acp");
    expect(formatAgentCommand(gemini)).toBe("gemini --acp");
    expect(formatAgentCommand(claudeAgent)).toBe("claude-agent-acp");
    expect(formatAgentCommand(claudeCodeRs)).toBe("claude-code-acp-rs --acp");
  });

  it("resolves ACP provider from command", () => {
    expect(resolveAcpProvider("agenthub-codex-acp")).toBe("codex");
    expect(resolveAcpProvider("/usr/local/bin/gemini")).toBe("gemini");
    expect(resolveAcpProvider("kimi")).toBe("kimi");
    expect(resolveAcpProvider("claude-agent-acp")).toBe("claude");
    expect(resolveAcpProvider("/opt/bin/claude-code-acp-rs")).toBe("claude");
    expect(resolveAcpProvider("unknown")).toBe(null);
  });

  it("formats agent model label from args or provider", () => {
    expect(formatAgentModelLabel("agenthub-codex-acp", [])).toBe("Codex");
    expect(formatAgentModelLabel("gemini", ["--model", "gemini-1.5-pro"]))
      .toBe("gemini-1.5-pro");
    expect(formatAgentModelLabel("kimi", ["--model=moonshot-v1"]))
      .toBe("moonshot-v1");
    expect(formatAgentModelLabel("claude-code-acp-rs", [])).toBe("Claude");
  });
});
