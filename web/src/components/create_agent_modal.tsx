import React from "react";
import {
  Alert,
  Button,
  NativeSelect,
  Group,
  List,
  Modal,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  TextInput,
} from "@mantine/core";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentCommand,
  getAgentPreset,
  isAgentPresetId,
  listAgentPresets,
  type AgentPresetId,
} from "../agent_presets";
import {
  CODEX_ACP_MODE_OPTIONS,
  DEFAULT_CODEX_ACP_MODE,
  formatCodexAcpModeLabel,
  normalizeCodexAcpModeId,
} from "../codex_acp_modes";
import {
  NOTION_MODAL_CLASSNAMES,
  NOTION_MODAL_OVERLAY_PROPS,
  TEAM_MODAL_CLASSNAMES,
} from "../ui/floating_surfaces";

export type CreateAgentModalProps = {
  title?: string;
  confirmLabel?: string;
  agentPresetLabel?: string;
  agentPresetSummaryLabel?: string;
  commandSummaryLabel?: string;
  showCommandSummary?: boolean;
  runtimeModeSectionLabel?: string;
  advancedOptionsLabel?: string;
  advancedOptionsHint?: string;
  teamStyled?: boolean;
  agentName: string;
  setAgentName: (value: string) => void;
  agentWorkdir: string;
  setAgentWorkdir: (value: string) => void;
  agentPresetId: AgentPresetId;
  setAgentPresetId: (value: AgentPresetId) => void;
  codexAcpDefaultMode?: string;
  setCodexAcpDefaultMode?: (value: string) => void;
  runtimeModel?: string;
  setRuntimeModel?: (value: string) => void;
  thinkingLevel?: string;
  setThinkingLevel?: (value: string) => void;
  worktreeMode: "use_existing" | "create_worktree" | "reuse_worktree";
  setWorktreeMode: (
    value: "use_existing" | "create_worktree" | "reuse_worktree"
  ) => void;
  worktreeRepo: string;
  setWorktreeRepo: (value: string) => void;
  worktreeRef: string;
  setWorktreeRef: (value: string) => void;
  codeMode: boolean;
  setCodeMode: (value: boolean) => void;
  worktreeError: string | null;
  showWorktreeAdvancedOptions?: boolean;
  createBusy: boolean;
  workdirPlaceholder?: string;
  withinPortal?: boolean;
  children?: React.ReactNode;
  onCreateAgent: () => void;
  onClose: () => void;
};

const worktreeOptions = [
  { value: "use_existing", label: "Use existing workdir" },
  { value: "create_worktree", label: "Create git worktree" },
  { value: "reuse_worktree", label: "Reuse git worktree" },
];
export const THINKING_LEVEL_OPTIONS = [
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "max", label: "Max" },
];
export const CODEX_RUNTIME_MODEL_OPTIONS = [
  { value: "gpt-5.6-sol", label: "GPT-5.6-Sol" },
  { value: "gpt-5.6-terra", label: "GPT-5.6-Terra" },
  { value: "gpt-5.6-luna", label: "GPT-5.6-Luna" },
  { value: "gpt-5.5", label: "GPT-5.5" },
  { value: "gpt-5.4", label: "GPT-5.4" },
  { value: "gpt-5.4-mini", label: "GPT-5.4-Mini" },
  { value: "gpt-5.2", label: "GPT-5.2" },
];
const TEAM_AGENT_MODAL_ACCENT_BUTTON_CLASS =
  "!border !border-ui-border-emphasis !bg-brand-primary !text-white !shadow-sm transition hover:!border-ui-border-strong hover:!bg-brand-primary-hover";
const TEAM_AGENT_MODAL_MUTED_BUTTON_CLASS =
  "!border !border-ui-border !bg-white !text-ui-text-primary !shadow-sm transition hover:!border-ui-border-emphasis hover:!bg-ui-surface-soft";
const TEAM_AGENT_MODAL_INFO_STRIP_CLASS =
  "overflow-hidden rounded-xl border border-ui-border bg-ui-surface shadow-sm";
const TEAM_AGENT_MODAL_INFO_STRIP_GRID_CLASS =
  "grid gap-px bg-ui-border sm:grid-cols-[minmax(0,1fr)_170px]";
const TEAM_AGENT_MODAL_INFO_STRIP_GRID_WITH_COMMAND_CLASS =
  "grid gap-px bg-ui-border sm:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)_170px]";
const TEAM_AGENT_MODAL_INFO_ITEM_CLASS =
  "min-w-0 bg-ui-surface px-3 py-2.5";
const TEAM_AGENT_MODAL_INFO_LABEL_CLASS =
  "text-[10px] font-semibold uppercase tracking-[0.14em] text-ui-text-muted";
const TEAM_AGENT_MODAL_INFO_VALUE_CLASS =
  "mt-1 text-[13px] leading-5 text-ui-text-primary";

export function resolveCreateAgentPresetId(value: string | null): AgentPresetId {
  if (value && isAgentPresetId(value)) {
    return value;
  }
  return DEFAULT_AGENT_PRESET_ID;
}

export function resolveCreateAgentWorktreeMode(
  value: string | null
): "use_existing" | "create_worktree" | "reuse_worktree" | null {
  if (
    value === "use_existing" ||
    value === "create_worktree" ||
    value === "reuse_worktree"
  ) {
    return value;
  }
  return null;
}

export function shouldAutoExpandCreateAgentAdvancedOptions(
  showWorktreeAdvancedOptions: boolean,
  worktreeMode: "use_existing" | "create_worktree" | "reuse_worktree",
  worktreeError: string | null
): boolean {
  return (
    showWorktreeAdvancedOptions &&
    (worktreeMode !== "use_existing" || Boolean(worktreeError))
  );
}

export function CreateAgentModal({
  title = "Create Agent",
  confirmLabel = "Create Agent",
  agentPresetLabel = "Agent preset",
  agentPresetSummaryLabel = "Preset",
  commandSummaryLabel = "Command",
  showCommandSummary = true,
  runtimeModeSectionLabel = "Mode",
  advancedOptionsLabel = "Advanced workspace options",
  advancedOptionsHint = "Worktree mode and git worktree parameters.",
  teamStyled = false,
  agentName,
  setAgentName,
  agentWorkdir,
  setAgentWorkdir,
  agentPresetId,
  setAgentPresetId,
  codexAcpDefaultMode = DEFAULT_CODEX_ACP_MODE,
  setCodexAcpDefaultMode,
  runtimeModel = "",
  setRuntimeModel,
  thinkingLevel = "",
  setThinkingLevel,
  worktreeMode,
  setWorktreeMode,
  worktreeRepo,
  setWorktreeRepo,
  worktreeRef,
  setWorktreeRef,
  codeMode,
  setCodeMode,
  worktreeError,
  showWorktreeAdvancedOptions = true,
  createBusy,
  workdirPlaceholder = "Workdir",
  withinPortal = true,
  children,
  onCreateAgent,
  onClose,
}: CreateAgentModalProps) {
  const [showAdvancedOptions, setShowAdvancedOptions] = React.useState(
    () =>
      showWorktreeAdvancedOptions &&
      (worktreeMode !== "use_existing" || Boolean(worktreeError))
  );
  const [customizeCreateWorkdir, setCustomizeCreateWorkdir] = React.useState(false);
  const presets = listAgentPresets();
  const preset = getAgentPreset(agentPresetId);
  const commandSummary = formatAgentCommand(preset);
  const isCodexPreset = preset.provider === "codex";
  const supportsRuntimeProfile =
    preset.provider === "codex" || preset.provider === "claude";
  const normalizedCodexAcpDefaultMode =
    normalizeCodexAcpModeId(codexAcpDefaultMode);
  const isCreateWorktreeMode = worktreeMode === "create_worktree";
  const normalizedDefaultRoot = workdirPlaceholder.trim().replace(/[\\/]+$/, "");
  const normalizedCurrentWorkdir = agentWorkdir.trim().replace(/[\\/]+$/, "");
  const hasCustomCreateWorkdir =
    normalizedCurrentWorkdir.length > 0 &&
    normalizedCurrentWorkdir !== normalizedDefaultRoot;
  const showWorkdirInput =
    !isCreateWorktreeMode || customizeCreateWorkdir || hasCustomCreateWorkdir;
  const shouldAutoExpandAdvancedOptions = shouldAutoExpandCreateAgentAdvancedOptions(
    showWorktreeAdvancedOptions,
    worktreeMode,
    worktreeError
  );
  const previousAutoExpandConditionRef = React.useRef(shouldAutoExpandAdvancedOptions);
  const presetOptions = presets.map((entry) => ({
    value: entry.id,
    label: entry.label,
  }));
  const runtimeModeValue = codeMode ? "Code" : "Chat";

  React.useEffect(() => {
    if (!isCreateWorktreeMode) {
      setCustomizeCreateWorkdir(false);
    }
  }, [isCreateWorktreeMode]);

  React.useEffect(() => {
    if (!showWorktreeAdvancedOptions) {
      previousAutoExpandConditionRef.current = false;
      return;
    }
    if (shouldAutoExpandAdvancedOptions && !previousAutoExpandConditionRef.current) {
      setShowAdvancedOptions(true);
    }
    previousAutoExpandConditionRef.current = shouldAutoExpandAdvancedOptions;
  }, [showWorktreeAdvancedOptions, shouldAutoExpandAdvancedOptions]);

  return (
    <Modal
      opened
      onClose={onClose}
      title={title}
      size="lg"
      radius="md"
      withCloseButton={false}
      closeOnEscape={false}
      closeOnClickOutside={false}
      withinPortal={withinPortal}
      classNames={teamStyled ? TEAM_MODAL_CLASSNAMES : NOTION_MODAL_CLASSNAMES}
      overlayProps={NOTION_MODAL_OVERLAY_PROPS}
    >
      <Stack gap="sm">
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <TextInput
            label="Name"
            placeholder="e.g. Alice"
            value={agentName}
            onChange={(event) => setAgentName(event.currentTarget.value)}
          />
          <NativeSelect
            label={agentPresetLabel}
            value={agentPresetId}
            data={presetOptions}
            onChange={(event) => {
              const nextPresetId = resolveCreateAgentPresetId(event.currentTarget.value);
              setAgentPresetId(nextPresetId);
              const nextPreset = getAgentPreset(nextPresetId);
              if (nextPreset.provider !== "codex" && nextPreset.provider !== "claude") {
                setRuntimeModel?.("");
                setThinkingLevel?.("");
              }
            }}
          />
          {isCodexPreset ? (
            <Select
              label="Codex permissions"
              description="Startup mode. Existing agents need a restart for changes to apply."
              value={normalizedCodexAcpDefaultMode}
              data={CODEX_ACP_MODE_OPTIONS}
              allowDeselect={false}
              onChange={(value) => {
                setCodexAcpDefaultMode?.(
                  normalizeCodexAcpModeId(value ?? DEFAULT_CODEX_ACP_MODE)
                );
              }}
            />
          ) : null}
          {supportsRuntimeProfile ? (
            <>
              {isCodexPreset ? (
                <Select
                  label="Runtime model"
                  description="Optional. Leave blank to use the provider default."
                  placeholder="Provider default"
                  value={runtimeModel || null}
                  data={CODEX_RUNTIME_MODEL_OPTIONS}
                  searchable
                  clearable
                  onChange={(value) => setRuntimeModel?.(value ?? "")}
                />
              ) : (
                <TextInput
                  label="Runtime model"
                  description="Optional. Leave blank to use the provider default."
                  placeholder="claude-sonnet"
                  value={runtimeModel}
                  onChange={(event) => setRuntimeModel?.(event.currentTarget.value)}
                />
              )}
              <Select
                label="Thinking level"
                description="Applied when the next session starts."
                placeholder="Provider default"
                value={thinkingLevel || null}
                data={THINKING_LEVEL_OPTIONS}
                clearable
                onChange={(value) => setThinkingLevel?.(value ?? "")}
              />
            </>
          ) : null}
          {showWorkdirInput ? (
            <TextInput
              label={isCreateWorktreeMode ? "Workdir (optional override)" : "Workspace path"}
              placeholder={workdirPlaceholder}
              value={agentWorkdir}
              onChange={(event) => setAgentWorkdir(event.currentTarget.value)}
            />
          ) : (
            <Stack gap={4}>
              <Text size="sm" fw={500}>
                Workspace path
              </Text>
              <Text size="sm" c="dimmed">
                Auto-create under: {workdirPlaceholder}
              </Text>
              <Button
                variant="subtle"
                size="compact-sm"
                px={0}
                w="fit-content"
                onClick={() => setCustomizeCreateWorkdir(true)}
              >
                Customize path
              </Button>
            </Stack>
          )}
        </SimpleGrid>

        {showWorktreeAdvancedOptions ? (
          <Stack gap={4}>
            <Button
              variant="subtle"
              size="compact-sm"
              px={0}
              w="fit-content"
              onClick={() => setShowAdvancedOptions((prev) => !prev)}
              aria-expanded={showAdvancedOptions}
            >
              {showAdvancedOptions
                ? `Hide ${advancedOptionsLabel}`
                : `Show ${advancedOptionsLabel}`}
            </Button>
            <Text size="xs" c="dimmed">
              {advancedOptionsHint}
            </Text>
          </Stack>
        ) : null}

        {showWorktreeAdvancedOptions && showAdvancedOptions ? (
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
            <Select
              label="Workspace mode"
              placeholder="Select worktree mode"
              value={worktreeMode}
              data={worktreeOptions}
              allowDeselect={false}
              onChange={(value) => {
                const nextMode = resolveCreateAgentWorktreeMode(value);
                if (nextMode) {
                  setWorktreeMode(nextMode);
                }
              }}
            />
            {worktreeMode === "create_worktree" ||
            worktreeMode === "reuse_worktree" ? (
              <TextInput
                label="Repository path"
                placeholder="Worktree repo path"
                value={worktreeRepo}
                onChange={(event) => setWorktreeRepo(event.currentTarget.value)}
              />
            ) : null}
            {worktreeMode === "create_worktree" ? (
              <TextInput
                label="Repository ref"
                placeholder="Branch or commit"
                value={worktreeRef}
                onChange={(event) => setWorktreeRef(event.currentTarget.value)}
              />
            ) : null}
          </SimpleGrid>
        ) : null}

        <div className={TEAM_AGENT_MODAL_INFO_STRIP_CLASS}>
          <div
            className={
              showCommandSummary
                ? TEAM_AGENT_MODAL_INFO_STRIP_GRID_WITH_COMMAND_CLASS
                : TEAM_AGENT_MODAL_INFO_STRIP_GRID_CLASS
            }
          >
            <div className={TEAM_AGENT_MODAL_INFO_ITEM_CLASS}>
              <p className={TEAM_AGENT_MODAL_INFO_LABEL_CLASS}>
                {agentPresetSummaryLabel}
              </p>
              <p className={TEAM_AGENT_MODAL_INFO_VALUE_CLASS}>{preset.label}</p>
            </div>
            {showCommandSummary ? (
              <div className={TEAM_AGENT_MODAL_INFO_ITEM_CLASS}>
                <p className={TEAM_AGENT_MODAL_INFO_LABEL_CLASS}>{commandSummaryLabel}</p>
                <p
                  className={`${TEAM_AGENT_MODAL_INFO_VALUE_CLASS} break-all font-mono text-[12px]`}
                >
                  {commandSummary || "Auto resolve from preset"}
                </p>
              </div>
            ) : null}
            <div className={`${TEAM_AGENT_MODAL_INFO_ITEM_CLASS} flex items-center justify-between gap-3`}>
              <div className="min-w-0">
                <p className={TEAM_AGENT_MODAL_INFO_LABEL_CLASS}>
                  {runtimeModeSectionLabel}
                </p>
                <p className={TEAM_AGENT_MODAL_INFO_VALUE_CLASS}>
                  {isCodexPreset
                    ? `${runtimeModeValue} · ${formatCodexAcpModeLabel(normalizedCodexAcpDefaultMode)}`
                    : runtimeModeValue}
                </p>
              </div>
              <Switch
                aria-label="Toggle code mode"
                checked={codeMode}
                onChange={(event) => setCodeMode(event.currentTarget.checked)}
              />
            </div>
          </div>
        </div>

        {children}

        {worktreeError ? (
          <Alert color="red" title="Worktree Setup Failed" variant="light">
            <Text size="sm">{worktreeError}</Text>
            <List size="sm" spacing="xs" mt="xs" withPadding>
              <List.Item>Check Safe Paths for the workdir and repo path.</List.Item>
              <List.Item>
                Ensure the workdir is empty when creating a worktree.
              </List.Item>
              <List.Item>
                Verify the git repo exists and the ref is valid.
              </List.Item>
            </List>
          </Alert>
        ) : null}

        <Group justify="flex-end" mt="xs">
          <Button
            variant="default"
            onClick={onClose}
            disabled={createBusy}
            className={TEAM_AGENT_MODAL_MUTED_BUTTON_CLASS}
          >
            Cancel
          </Button>
          <Button
            onClick={onCreateAgent}
            loading={createBusy}
            disabled={createBusy}
            className={TEAM_AGENT_MODAL_ACCENT_BUTTON_CLASS}
          >
            {confirmLabel}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
