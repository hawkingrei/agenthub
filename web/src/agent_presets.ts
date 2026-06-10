export type AgentPresetId =
  | "codex"
  | "gemini"
  | "kimi"
  | "claude"
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
    id: "claude",
    label: "Claude ACP",
    command: "agenthub-claude-acp",
    args: [],
    provider: "claude",
  },
  {
    id: "claude_agent",
    label: "Claude Agent ACP (external)",
    command: "claude-agent-acp",
    args: [],
    provider: "claude",
  },
  {
    id: "claude_code_rs",
    label: "Claude Code ACP (external Rust)",
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
  ["agenthub-claude-acp", "claude"],
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
  const name = commandBinaryName(command);
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
  const name = commandBinaryName(command);
  return name || null;
}

function commandBinaryName(command: string): string | null {
  const token = command.trim().split(/\s+/, 1)[0]?.trim();
  return token?.split(/[\\/]/).pop()?.trim() || null;
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
