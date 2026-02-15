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
  createBusy: boolean;
  workdirPlaceholder?: string;
  withinPortal?: boolean;
  onCreateAgent: () => void;
  onClose: () => void;
};

const worktreeOptions = [
  { value: "use_existing", label: "Use existing workdir" },
  { value: "create_worktree", label: "Create git worktree" },
  { value: "reuse_worktree", label: "Reuse git worktree" },
];

export function CreateAgentModal({
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
  createBusy,
  workdirPlaceholder = "Workdir",
  withinPortal = true,
  onCreateAgent,
  onClose,
}: CreateAgentModalProps) {
  const presets = listAgentPresets();
  const preset = getAgentPreset(agentPresetId);
  const commandSummary = formatAgentCommand(preset);
  const presetOptions = presets.map((entry) => ({
    value: entry.id,
    label: entry.label,
  }));

  return (
    <Modal
      opened
      onClose={onClose}
      title="Create Agent"
      size="lg"
      radius="md"
      withCloseButton={false}
      closeOnEscape={false}
      closeOnClickOutside={false}
      withinPortal={withinPortal}
      overlayProps={{ backgroundOpacity: 0.35, blur: 2 }}
    >
      <Stack gap="sm">
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <TextInput
            label="Agent name"
            placeholder="Agent name"
            value={agentName}
            onChange={(event) => setAgentName(event.currentTarget.value)}
          />
          <TextInput
            label="Workdir"
            placeholder={workdirPlaceholder}
            value={agentWorkdir}
            onChange={(event) => setAgentWorkdir(event.currentTarget.value)}
          />
          <Select
            label="Worktree mode"
            placeholder="Select worktree mode"
            value={worktreeMode}
            data={worktreeOptions}
            allowDeselect={false}
            onChange={(value) => {
              if (
                value === "use_existing" ||
                value === "create_worktree" ||
                value === "reuse_worktree"
              ) {
                setWorktreeMode(value);
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
          <Select
            label="Agent preset"
            placeholder="Select preset"
            value={agentPresetId}
            data={presetOptions}
            allowDeselect={false}
            onChange={(value) => {
              if (value && isAgentPresetId(value)) {
                setAgentPresetId(value);
                return;
              }
              setAgentPresetId(DEFAULT_AGENT_PRESET_ID);
            }}
          />
        </SimpleGrid>

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
          >
            Create Agent
          </Button>
          <Button variant="default" onClick={onClose} disabled={createBusy}>
            Cancel
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
