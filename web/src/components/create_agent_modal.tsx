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

type CreateAgentModalProps = {
  agentName: string;
  setAgentName: (value: string) => void;
  agentWorkdir: string;
  setAgentWorkdir: (value: string) => void;
  agentCommand: string;
  setAgentCommand: (value: string) => void;
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
  withinPortal?: boolean;
  onCreateAgent: () => void;
  onClose: () => void;
};

const worktreeOptions = [
  { value: "use_existing", label: "Use existing workdir" },
  { value: "create_worktree", label: "Create git worktree" },
  { value: "reuse_worktree", label: "Reuse git worktree" },
];

const commandOptions = [
  { value: "agenthub-codex-acp", label: "agenthub-codex-acp" },
];

export function CreateAgentModal({
  agentName,
  setAgentName,
  agentWorkdir,
  setAgentWorkdir,
  agentCommand,
  setAgentCommand,
  worktreeMode,
  setWorktreeMode,
  worktreeRepo,
  setWorktreeRepo,
  worktreeRef,
  setWorktreeRef,
  codeMode,
  setCodeMode,
  worktreeError,
  withinPortal = true,
  onCreateAgent,
  onClose,
}: CreateAgentModalProps) {
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
            placeholder="Workdir"
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
            label="Agent command"
            placeholder="Select command"
            value={agentCommand}
            data={commandOptions}
            allowDeselect={false}
            onChange={(value) => {
              if (value) {
                setAgentCommand(value);
              }
            }}
          />
        </SimpleGrid>

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
          <Button onClick={onCreateAgent}>Create Agent</Button>
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
