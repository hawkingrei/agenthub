export type AgentPresetId = "codex" | "gemini" | "kimi";

export type AgentPreset = {
  id: AgentPresetId;
  label: string;
  command: string;
  args: string[];
  provider: string;
};

const PRESETS: AgentPreset[] = [
  {
    id: "codex",
    label: "Codex ACP",
    command: "agenthub-codex-acp",
    args: [],
    provider: "codex",
  },
  {
    id: "gemini",
    label: "Gemini CLI",
    command: "gemini",
    args: ["--experimental-acp"],
    provider: "gemini",
  },
  {
    id: "kimi",
    label: "Kimi CLI",
    command: "kimi",
    args: ["acp"],
    provider: "kimi",
  },
];

const PRESET_MAP = new Map<AgentPresetId, AgentPreset>(
  PRESETS.map((preset) => [preset.id, preset])
);

export const DEFAULT_AGENT_PRESET_ID: AgentPresetId = "codex";

export function listAgentPresets(): AgentPreset[] {
  return PRESETS.slice();
}

export function isAgentPresetId(value: string): value is AgentPresetId {
  return PRESET_MAP.has(value as AgentPresetId);
}

export function getAgentPreset(id: AgentPresetId): AgentPreset {
  return PRESET_MAP.get(id) ?? PRESET_MAP.get("codex")!;
}

export function formatAgentCommand(preset: AgentPreset): string {
  if (!preset.args.length) {
    return preset.command;
  }
  return `${preset.command} ${preset.args.join(" ")}`;
}

export function resolveAcpProvider(command: string): string | null {
  const name = command.split(/[\\/]/).pop()?.trim();
  if (!name) return null;
  if (name === "agenthub-codex-acp" || name === "codex-acp") return "codex";
  if (name === "gemini") return "gemini";
  if (name === "kimi") return "kimi";
  return null;
}
