export const DEFAULT_CODEX_ACP_MODE = "full-access";

export const CODEX_ACP_MODE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "full-access", label: "Yolo / full access" },
  { value: "auto", label: "Auto" },
  { value: "read-only", label: "Read only" },
];

const CODEX_ACP_MODE_ALIASES: Record<string, string> = {
  yolo: DEFAULT_CODEX_ACP_MODE,
  yalo: DEFAULT_CODEX_ACP_MODE,
  danger_full_access: DEFAULT_CODEX_ACP_MODE,
  "danger-full-access": DEFAULT_CODEX_ACP_MODE,
};

const CODEX_ACP_MODE_LABELS = new Map(
  CODEX_ACP_MODE_OPTIONS.map((option) => [option.value, option.label])
);

export function normalizeCodexAcpModeId(value?: string | null): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    return DEFAULT_CODEX_ACP_MODE;
  }
  const aliased = CODEX_ACP_MODE_ALIASES[trimmed];
  if (aliased) {
    return aliased;
  }
  if (CODEX_ACP_MODE_LABELS.has(trimmed)) {
    return trimmed;
  }
  return DEFAULT_CODEX_ACP_MODE;
}

export function formatCodexAcpModeLabel(value?: string | null): string {
  const normalized = normalizeCodexAcpModeId(value);
  return CODEX_ACP_MODE_LABELS.get(normalized) ?? CODEX_ACP_MODE_OPTIONS[0].label;
}
