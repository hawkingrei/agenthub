import React from "react";
import { Alert, Button, Group, Select, Stack, Text, TextInput } from "@mantine/core";
import { AgentNodeRecord, AgentNodeUpdate, AgentRecord } from "../api";

type AgentNodeDraft = {
  name: string;
  grpcTarget: string;
  tlsServerName: string;
  defaultWorktreeRoot: string;
};

const RESERVED_AGENT_NODE_ID = "main";

type AgentNodeSectionProps = {
  nodes: AgentNodeRecord[];
  agents: AgentRecord[];
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
  const nodeId = input.nodeId.trim();
  if (!nodeId) {
    return "Node ID is required.";
  }
  if (nodeId === RESERVED_AGENT_NODE_ID) {
    return `Node ID '${RESERVED_AGENT_NODE_ID}' is reserved.`;
  }
  if (nodeId.length > 128) {
    return "Node ID must be at most 128 characters.";
  }
  if (![...nodeId].every((ch) => /[A-Za-z0-9._:-]/.test(ch))) {
    return "Node ID may only contain ASCII letters, numbers, '.', '_', '-', or ':'.";
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
  agents,
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
  const availableNodes = React.useMemo(
    () =>
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
          ],
    [nodes]
  );
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
  const agentsByNodeId = React.useMemo(() => {
    const buckets = new Map<string, AgentRecord[]>();
    for (const agent of agents) {
      const nodeId = agent.target_node_id?.trim() || "main";
      const current = buckets.get(nodeId) ?? [];
      current.push(agent);
      buckets.set(nodeId, current);
    }
    return buckets;
  }, [agents]);
  const remoteNodes = React.useMemo(
    () => availableNodes.filter((node) => !node.is_main),
    [availableNodes]
  );
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
      <div className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-4 shadow-sm">
        <Stack gap="sm">
          <div>
            <Text fw={600}>Machines &amp; Agents</Text>
            <Text size="xs" c="dimmed">
              Pick where the new agent should run, and see which agents are already attached to
              each machine.
            </Text>
          </div>
          <div className="grid gap-2">
            {availableNodes.map((node) => {
              const nodeAgents = agentsByNodeId.get(node.id) ?? [];
              const isSelected = selectedNode?.id === node.id;
              return (
                <button
                  key={node.id}
                  type="button"
                  className={`w-full rounded-xl border px-3 py-3 text-left transition ${
                    isSelected
                      ? "border-ui-border-emphasis bg-white shadow-sm"
                      : "border-ui-border bg-white/70 hover:border-ui-border-emphasis hover:bg-white"
                  }`}
                  onClick={() => onTargetNodeIdChange(node.id)}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <Text size="sm" fw={600}>
                          {node.name}
                        </Text>
                        <Text size="xs" c="dimmed">
                          {node.is_main ? "local" : "remote"}
                        </Text>
                        <Text size="xs" c="dimmed">
                          {nodeAgents.length} agent{nodeAgents.length === 1 ? "" : "s"}
                        </Text>
                      </div>
                      <Text size="xs" c="dimmed" mt={4}>
                        {node.is_main
                          ? "Run on this AgentHub instance."
                          : node.grpc_target ?? "Encrypted gRPC target"}
                      </Text>
                      {node.default_worktree_root ? (
                        <Text size="xs" c="dimmed" mt={4}>
                          Worktree root: {node.default_worktree_root}
                        </Text>
                      ) : null}
                    </div>
                    {isSelected ? (
                      <Text size="xs" fw={600}>
                        Selected
                      </Text>
                    ) : null}
                  </div>
                  {nodeAgents.length > 0 ? (
                    <div className="mt-3 flex flex-wrap gap-1.5">
                      {nodeAgents.slice(0, 4).map((agent) => (
                        <span
                          key={agent.id}
                          className="inline-flex items-center rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-[11px] text-ui-text-secondary"
                        >
                          {agent.name}
                        </span>
                      ))}
                      {nodeAgents.length > 4 ? (
                        <span className="inline-flex items-center rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-[11px] text-ui-text-secondary">
                          +{nodeAgents.length - 4} more
                        </span>
                      ) : null}
                    </div>
                  ) : (
                    <Text size="xs" c="dimmed" mt={10}>
                      No agents assigned yet.
                    </Text>
                  )}
                </button>
              );
            })}
          </div>
        </Stack>
      </div>

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
