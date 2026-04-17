import React from "react";
import { Alert, Button, Group, Select, Stack, Text, TextInput } from "@mantine/core";
import {
  AgentNodeJoinBootstrapInfo,
  AgentNodeRecord,
  AgentNodeUpdate,
  AgentRecord,
} from "../api";
import { SelectableListItem } from "../ui/primitives";
import { validateAgentNodeDraft, validateAgentNodeUpdateDraft } from "./agent_node_validation";

type AgentNodeDraft = {
  name: string;
  grpcTarget: string;
  tlsServerName: string;
  defaultWorktreeRoot: string;
};

export type AgentNodeSectionProps = {
  nodes: AgentNodeRecord[];
  agents: AgentRecord[];
  nodeJoinBootstrap: AgentNodeJoinBootstrapInfo | null;
  nodeJoinBootstrapLoading: boolean;
  nodeJoinBootstrapError: string | null;
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

export function AgentNodeSection({
  nodes,
  agents,
  nodeJoinBootstrap,
  nodeJoinBootstrapLoading,
  nodeJoinBootstrapError,
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
            <Text fw={600}>Join node with token</Text>
            <Text size="xs" c="dimmed">
              Use a copy/paste bootstrap token on the remote node. QR onboarding is not used for
              Agent Node join. After the node can reach the control plane, register its gRPC target
              below.
            </Text>
          </div>
          {nodeJoinBootstrapLoading ? (
            <Text size="xs" c="dimmed">
              Loading node bootstrap details...
            </Text>
          ) : nodeJoinBootstrapError ? (
            <Alert color="red" variant="light" title="Bootstrap unavailable">
              <Text size="sm">{nodeJoinBootstrapError}</Text>
            </Alert>
          ) : nodeJoinBootstrap ? (
            nodeJoinBootstrap.enabled ? (
            <Stack gap="sm">
              <Alert color="blue" variant="light" title="Token-based node bootstrap">
                <Text size="sm">
                  Copy this token into the remote node&apos;s
                  {" "}
                  <code>[internal_grpc.bootstrap].token</code>
                  {" "}
                  setting, then restart the node and register the reachable gRPC target here.
                </Text>
              </Alert>
              <Group grow align="end">
                <TextInput
                  label="Join token"
                  value={nodeJoinBootstrap.bootstrap_token ?? ""}
                  readOnly
                />
                <TextInput
                  label="Control-plane gRPC listen"
                  value={nodeJoinBootstrap.grpc_listen_addr ?? ""}
                  readOnly
                />
              </Group>
              <Group grow align="end">
                <TextInput
                  label="Security mode"
                  value={nodeJoinBootstrap.security_mode ?? ""}
                  readOnly
                />
                <TextInput
                  label="Issuer"
                  value={nodeJoinBootstrap.issuer ?? ""}
                  readOnly
                />
                <TextInput
                  label="Audience"
                  value={nodeJoinBootstrap.audience ?? ""}
                  readOnly
                />
              </Group>
              <TextInput
                label="Cert directory"
                value={nodeJoinBootstrap.cert_dir ?? ""}
                readOnly
              />
              <Text size="xs" c="dimmed">
                If the listen address uses
                {" "}
                <code>0.0.0.0</code>
                {" "}
                or
                {" "}
                <code>localhost</code>
                , replace it with a host or IP that the remote node can actually reach.
              </Text>
            </Stack>
          ) : (
            <Alert color="yellow" variant="light" title="Internal gRPC required">
              <Text size="sm">
                Enable
                {" "}
                <code>[internal_grpc]</code>
                {" "}
                on the main control plane before joining remote nodes by token.
              </Text>
            </Alert>
            )
          ) : (
            <Text size="xs" c="dimmed">
              Agent Node Join Bootstrap details are not available yet.
            </Text>
          )}
        </Stack>
      </div>

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
                <SelectableListItem
                  key={node.id}
                  aria-pressed={isSelected}
                  active={isSelected}
                  className="w-full px-3 py-3"
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
                </SelectableListItem>
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
