import React from "react";
import {
  Alert,
  Button,
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

type CreateAgentModalProps = {
  title?: string;
  confirmLabel?: string;
  agentName: string;
  setAgentName: (value: string) => void;
  agentWorkdir: string;
  setAgentWorkdir: (value: string) => void;
  agentPresetId: AgentPresetId;
  setAgentPresetId: (value: AgentPresetId) => void;
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
const TEAM_AGENT_MODAL_CONTENT_STYLE = {
  backgroundColor: "#f6efe5",
  border: "3px solid #000",
  boxShadow: "0 4px 0 rgba(0, 0, 0, 0.18)",
};
const TEAM_AGENT_MODAL_HEADER_STYLE = {
  backgroundColor: "transparent",
  borderBottom: "2px solid #000",
  paddingBottom: "12px",
};
const TEAM_AGENT_MODAL_TITLE_STYLE = {
  color: "#000",
  fontSize: "0.85rem",
  fontWeight: 800,
  letterSpacing: "0.18em",
  textTransform: "uppercase" as const,
};
const TEAM_AGENT_MODAL_BODY_STYLE = {
  paddingTop: "16px",
};
const TEAM_AGENT_MODAL_ACCENT_BUTTON_CLASS =
  "!border-[3px] !border-black !bg-[#243243] !text-white !shadow-[0_2px_0_rgba(0,0,0,0.16)] transition hover:!-translate-y-[1px] hover:!shadow-[0_1px_0_rgba(0,0,0,0.12)]";
const TEAM_AGENT_MODAL_MUTED_BUTTON_CLASS =
  "!border-[3px] !border-black !bg-white !text-black !shadow-[0_2px_0_rgba(0,0,0,0.16)] transition hover:!-translate-y-[1px] hover:!shadow-[0_1px_0_rgba(0,0,0,0.12)]";

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
  agentName,
  setAgentName,
  agentWorkdir,
  setAgentWorkdir,
  agentPresetId,
  setAgentPresetId,
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
      overlayProps={{ backgroundOpacity: 0.35, blur: 2 }}
      styles={{
        content: TEAM_AGENT_MODAL_CONTENT_STYLE,
        header: TEAM_AGENT_MODAL_HEADER_STYLE,
        title: TEAM_AGENT_MODAL_TITLE_STYLE,
        body: TEAM_AGENT_MODAL_BODY_STYLE,
      }}
    >
      <Stack gap="sm">
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <TextInput
            label="Agent name"
            placeholder="Agent name"
            value={agentName}
            onChange={(event) => setAgentName(event.currentTarget.value)}
          />
          <Select
            label="Agent preset"
            placeholder="Select preset"
            value={agentPresetId}
            data={presetOptions}
            allowDeselect={false}
            onChange={(value) => {
              setAgentPresetId(resolveCreateAgentPresetId(value));
            }}
          />
          {showWorkdirInput ? (
            <TextInput
              label={isCreateWorktreeMode ? "Workdir (optional override)" : "Workdir"}
              placeholder={workdirPlaceholder}
              value={agentWorkdir}
              onChange={(event) => setAgentWorkdir(event.currentTarget.value)}
            />
          ) : (
            <Stack gap={4}>
              <Text size="sm" fw={500}>
                Workdir
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
              {showAdvancedOptions ? "Hide Advanced Options" : "Show Advanced Options"}
            </Button>
            <Text size="xs" c="dimmed">
              Worktree mode and git worktree parameters.
            </Text>
          </Stack>
        ) : null}

        {showWorktreeAdvancedOptions && showAdvancedOptions ? (
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
            <Select
              label="Worktree mode"
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
                label="Worktree repo path"
                placeholder="Worktree repo path"
                value={worktreeRepo}
                onChange={(event) => setWorktreeRepo(event.currentTarget.value)}
              />
            ) : null}
            {worktreeMode === "create_worktree" ? (
              <TextInput
                label="Worktree ref"
                placeholder="Worktree ref (branch or commit)"
                value={worktreeRef}
                onChange={(event) => setWorktreeRef(event.currentTarget.value)}
              />
            ) : null}
          </SimpleGrid>
        ) : null}

        {commandSummary ? (
          <Text size="sm" c="dimmed">
            Command: {commandSummary}
          </Text>
        ) : null}

        <Switch
          label="Code mode"
          checked={codeMode}
          onChange={(event) => setCodeMode(event.currentTarget.checked)}
        />

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
            onClick={onCreateAgent}
            loading={createBusy}
            disabled={createBusy}
            className={TEAM_AGENT_MODAL_ACCENT_BUTTON_CLASS}
          >
            {confirmLabel}
          </Button>
          <Button
            variant="default"
            onClick={onClose}
            disabled={createBusy}
            className={TEAM_AGENT_MODAL_MUTED_BUTTON_CLASS}
          >
            Cancel
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
