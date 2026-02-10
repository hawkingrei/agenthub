import React from "react";
import {
  Alert,
  Button,
  Group,
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
            <ul>
              <li>Check Safe Paths for the workdir and repo path.</li>
              <li>Ensure the workdir is empty when creating a worktree.</li>
              <li>Verify the git repo exists and the ref is valid.</li>
            </ul>
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
