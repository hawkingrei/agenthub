export type AgentPresetId =
  | "codex"
  | "gemini"
  | "kimi"
  | "claude_agent"
  | "claude_code_rs";

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
    args: ["--acp"],
    provider: "gemini",
  },
  {
    id: "kimi",
    label: "Kimi CLI",
    command: "kimi",
    args: ["acp"],
    provider: "kimi",
  },
  {
    id: "claude_agent",
    label: "Claude Agent ACP",
    command: "claude-agent-acp",
    args: [],
    provider: "claude",
  },
  {
    id: "claude_code_rs",
    label: "Claude Code ACP (Rust)",
    command: "claude-code-acp-rs",
    args: ["--acp"],
    provider: "claude",
  },
];

const PRESET_MAP = new Map<AgentPresetId, AgentPreset>(
  PRESETS.map((preset) => [preset.id, preset])
);

const COMMAND_PROVIDER_MAP = new Map<string, string>([
  ["agenthub-codex-acp", "codex"],
  ["codex-acp", "codex"],
  ["gemini", "gemini"],
  ["kimi", "kimi"],
  ["claude-agent-acp", "claude"],
  ["claude-code-acp-rs", "claude"],
]);

const PROVIDER_MODEL_LABELS = new Map<string, string>([
  ["codex", "Codex"],
  ["gemini", "Gemini"],
  ["kimi", "Kimi"],
  ["claude", "Claude"],
]);

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
  return COMMAND_PROVIDER_MAP.get(name) ?? null;
}

export function formatAgentModelLabel(
  command: string,
  args: string[]
): string | null {
  const model = extractArgValue(args, ["--model", "-m"]);
  if (model) return model;
  const provider = resolveAcpProvider(command);
  if (provider) {
    return PROVIDER_MODEL_LABELS.get(provider) ?? provider;
  }
  const name = command.split(/[\\/]/).pop()?.trim();
  return name || null;
}

function extractArgValue(args: string[], flags: string[]): string | null {
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    for (const flag of flags) {
      if (arg === flag) {
        const next = args[i + 1];
        if (next && !next.startsWith("-")) {
          return next;
        }
      }
      const prefix = `${flag}=`;
      if (arg.startsWith(prefix)) {
        const value = arg.slice(prefix.length).trim();
        if (value) return value;
      }
    }
  }
  return null;
}
