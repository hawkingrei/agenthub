import React from "react";
import { Alert, Button, Group, Select, Stack, Text, TextInput } from "@mantine/core";
import { AgentNodeRecord, AgentNodeUpdate } from "../api";

type AgentNodeDraft = {
  name: string;
  grpcTarget: string;
  tlsServerName: string;
  defaultWorktreeRoot: string;
};

type AgentNodeSectionProps = {
  nodes: AgentNodeRecord[];
  targetNodeId: string;
  onTargetNodeIdChange: (value: string) => void;
  nodeIdInput: string;
  onNodeIdInputChange: (value: string) => void;
  nodeNameInput: string;
  onNodeNameInputChange: (value: string) => void;
  grpcTargetInput: string;
  onGrpcTargetInputChange: (value: string) => void;
  tlsServerNameInput: string;
  onTlsServerNameInputChange: (value: string) => void;
  defaultWorktreeRootInput: string;
  onDefaultWorktreeRootInputChange: (value: string) => void;
  createBusy: boolean;
  updatingNodeIds: Record<string, boolean>;
  deletingNodeIds: Record<string, boolean>;
  onCreateNode: () => void;
  onUpdateNode: (nodeId: string, payload: AgentNodeUpdate) => void;
  onDeleteNode: (nodeId: string) => void;
};

function toNodeDraft(node: AgentNodeRecord): AgentNodeDraft {
  return {
    name: node.name,
    grpcTarget: node.grpc_target ?? "",
    tlsServerName: node.tls_server_name ?? "",
    defaultWorktreeRoot: node.default_worktree_root ?? "",
  };
}

function validateAgentNodeMutableFields(input: {
  nodeName: string;
  grpcTarget: string;
}): string | null {
  if (!input.nodeName.trim()) {
    return "Node name is required.";
  }
  if (!input.grpcTarget.trim()) {
    return "gRPC target is required.";
  }
  return null;
}

export function validateAgentNodeDraft(input: {
  nodeId: string;
  nodeName: string;
  grpcTarget: string;
}): string | null {
  if (!input.nodeId.trim()) {
    return "Node ID is required.";
  }
  return validateAgentNodeMutableFields(input);
}

export function validateAgentNodeUpdateDraft(input: {
  nodeName: string;
  grpcTarget: string;
}): string | null {
  return validateAgentNodeMutableFields(input);
}

export function AgentNodeSection({
  nodes,
  targetNodeId,
  onTargetNodeIdChange,
  nodeIdInput,
  onNodeIdInputChange,
  nodeNameInput,
  onNodeNameInputChange,
  grpcTargetInput,
  onGrpcTargetInputChange,
  tlsServerNameInput,
  onTlsServerNameInputChange,
  defaultWorktreeRootInput,
  onDefaultWorktreeRootInputChange,
  createBusy,
  updatingNodeIds,
  deletingNodeIds,
  onCreateNode,
  onUpdateNode,
  onDeleteNode,
}: AgentNodeSectionProps) {
  const createNodeError = validateAgentNodeDraft({
    nodeId: nodeIdInput,
    nodeName: nodeNameInput,
    grpcTarget: grpcTargetInput,
  });
  const availableNodes =
    nodes.length > 0
      ? nodes
      : [
          {
            id: "main",
            name: "Main Node",
            grpc_target: null,
            tls_server_name: null,
            default_worktree_root: null,
            is_main: true,
            created_at: 0,
            updated_at: 0,
          },
        ];
  const resolvedTargetNodeId = targetNodeId.trim() || "main";
  const selectedNode =
    availableNodes.find((node) => node.id === resolvedTargetNodeId) ??
    availableNodes.find((node) => node.is_main) ??
    null;
  const nodeOptions = availableNodes.map((node) => ({
    value: node.id,
    label: node.is_main
      ? `${node.name} · local`
      : `${node.name} · ${node.grpc_target ?? "gRPC"}`,
  }));
  const remoteNodes = availableNodes.filter((node) => !node.is_main);
  const [editDrafts, setEditDrafts] = React.useState<Record<string, AgentNodeDraft>>({});

  React.useEffect(() => {
    setEditDrafts((prev) => {
      const next: Record<string, AgentNodeDraft> = {};
      let changed = false;
      for (const node of remoteNodes) {
        if (prev[node.id]) {
          next[node.id] = prev[node.id];
        } else {
          next[node.id] = toNodeDraft(node);
          changed = true;
        }
      }
      const prevKeys = Object.keys(prev);
      if (prevKeys.length !== Object.keys(next).length) {
        changed = true;
      }
      return changed ? next : prev;
    });
  }, [remoteNodes]);

  return (
    <Stack gap="sm">
      <Select
        label="Execution node"
        placeholder="Select node"
        value={selectedNode?.id ?? "main"}
        data={nodeOptions}
        allowDeselect={false}
        onChange={(value) => onTargetNodeIdChange(value ?? "main")}
      />

      <Alert
        color={selectedNode?.is_main ? "blue" : "grape"}
        variant="light"
        title={selectedNode?.is_main ? "Main node" : "Remote node"}
      >
        <Text size="sm">
          {selectedNode?.is_main
            ? "Run on this AgentHub instance. Local safe-path and worktree policies apply directly."
            : `Bind the agent to ${selectedNode?.name ?? "the remote node"} via encrypted gRPC${selectedNode?.tls_server_name ? ` (${selectedNode.tls_server_name})` : ""}.`}
        </Text>
        {!selectedNode?.is_main && selectedNode?.default_worktree_root ? (
          <Text size="sm" mt={6}>
            Blank create-worktree workdirs default to: {selectedNode.default_worktree_root}
          </Text>
        ) : null}
      </Alert>

      <div className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-4 shadow-sm">
        <Stack gap="sm">
          <div>
            <Text fw={600}>Register node</Text>
            <Text size="xs" c={createNodeError ? "red" : "dimmed"}>
              {createNodeError ??
                "AgentHub to node and node to node traffic uses encrypted gRPC."}
            </Text>
          </div>

          <Group grow align="end">
            <TextInput
              label="Node ID"
              placeholder="node-east"
              value={nodeIdInput}
              onChange={(event) => onNodeIdInputChange(event.currentTarget.value)}
            />
            <TextInput
              label="Node name"
              placeholder="Node East"
              value={nodeNameInput}
              onChange={(event) => onNodeNameInputChange(event.currentTarget.value)}
            />
          </Group>

          <Group grow align="end">
            <TextInput
              label="gRPC target"
              placeholder="https://node-east.internal:50051"
              value={grpcTargetInput}
              onChange={(event) => onGrpcTargetInputChange(event.currentTarget.value)}
            />
            <TextInput
              label="TLS server name"
              placeholder="node-east.internal"
              value={tlsServerNameInput}
              onChange={(event) => onTlsServerNameInputChange(event.currentTarget.value)}
            />
          </Group>

          <TextInput
            label="Default worktree root"
            placeholder="~/.agenthub/worktrees/node-east"
            value={defaultWorktreeRootInput}
            onChange={(event) => onDefaultWorktreeRootInputChange(event.currentTarget.value)}
            description="Optional. Used when remote create-worktree agents leave Workdir blank."
          />

          <Group justify="flex-end">
            <Button
              onClick={onCreateNode}
              loading={createBusy}
              disabled={createBusy || createNodeError !== null}
            >
              Add Node
            </Button>
          </Group>

          <Stack gap="xs">
            <Text size="sm" fw={500}>
              Registered remote nodes
            </Text>
            {remoteNodes.length === 0 ? (
              <Text size="xs" c="dimmed">
                No remote nodes registered yet.
              </Text>
            ) : (
              remoteNodes.map((node) => {
                const draft = editDrafts[node.id] ?? toNodeDraft(node);
                const updateError = validateAgentNodeUpdateDraft({
                  nodeName: draft.name,
                  grpcTarget: draft.grpcTarget,
                });
                return (
                  <div
                    key={node.id}
                    className="rounded-xl border border-ui-border bg-white px-3 py-3"
                  >
                    <Stack gap="sm">
                      <Group justify="space-between" align="flex-start">
                        <div className="min-w-0">
                          <Text size="sm" fw={600}>
                            {node.id}
                          </Text>
                          <Text size="xs" c="dimmed">
                            Update routing and remote worktree defaults for this node.
                          </Text>
                        </div>
                        <Text size="xs" c="dimmed">
                          {new Date(node.updated_at * 1000).toLocaleString()}
                        </Text>
                      </Group>

                      <Group grow align="end">
                        <TextInput
                          label="Node name"
                          value={draft.name}
                          onChange={(event) =>
                            setEditDrafts((prev) => ({
                              ...prev,
                              [node.id]: {
                                ...draft,
                                name: event.currentTarget.value,
                              },
                            }))
                          }
                        />
                        <TextInput
                          label="gRPC target"
                          value={draft.grpcTarget}
                          onChange={(event) =>
                            setEditDrafts((prev) => ({
                              ...prev,
                              [node.id]: {
                                ...draft,
                                grpcTarget: event.currentTarget.value,
                              },
                            }))
                          }
                        />
                      </Group>

                      <Group grow align="end">
                        <TextInput
                          label="TLS server name"
                          value={draft.tlsServerName}
                          onChange={(event) =>
                            setEditDrafts((prev) => ({
                              ...prev,
                              [node.id]: {
                                ...draft,
                                tlsServerName: event.currentTarget.value,
                              },
                            }))
                          }
                        />
                        <TextInput
                          label="Default worktree root"
                          placeholder="Optional"
                          value={draft.defaultWorktreeRoot}
                          onChange={(event) =>
                            setEditDrafts((prev) => ({
                              ...prev,
                              [node.id]: {
                                ...draft,
                                defaultWorktreeRoot: event.currentTarget.value,
                              },
                            }))
                          }
                        />
                      </Group>

                      <Group justify="space-between" align="center">
                        <Text size="xs" c={updateError ? "red" : "dimmed"}>
                          {updateError ??
                            "Leave Default worktree root blank to require explicit remote workdir."}
                        </Text>
                        <Group gap="xs">
                          <Button
                            variant="light"
                            size="xs"
                            loading={Boolean(updatingNodeIds[node.id])}
                            disabled={Boolean(updatingNodeIds[node.id]) || updateError !== null}
                            onClick={() =>
                              onUpdateNode(node.id, {
                                name: draft.name.trim(),
                                grpc_target: draft.grpcTarget.trim(),
                                tls_server_name: draft.tlsServerName.trim() || null,
                                default_worktree_root:
                                  draft.defaultWorktreeRoot.trim() || null,
                              })
                            }
                          >
                            Save
                          </Button>
                          <Button
                            variant="light"
                            color="red"
                            size="xs"
                            loading={Boolean(deletingNodeIds[node.id])}
                            disabled={Boolean(deletingNodeIds[node.id])}
                            onClick={() => onDeleteNode(node.id)}
                          >
                            Delete
                          </Button>
                        </Group>
                      </Group>
                    </Stack>
                  </div>
                );
              })
            )}
          </Stack>
        </Stack>
      </div>
    </Stack>
  );
}
