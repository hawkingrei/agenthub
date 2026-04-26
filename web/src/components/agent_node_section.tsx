import React from "react";
import { Alert, Button, Group, Stack, Text, TextInput } from "@mantine/core";
import {
  AgentNodeJoinBootstrapInfo,
  AgentNodeRecord,
  AgentNodeUpdate,
  AgentRecord,
} from "../api";
import {
  Badge,
  EmptyState,
  SelectableListItem,
} from "../ui/primitives";
import { validateAgentNodeDraft, validateAgentNodeUpdateDraft } from "./agent_node_validation";
import {
  AgentNodeDetailCard,
  describeAgentAttachment,
  describeSelectedNode,
  formatNodeTimestamp as formatTimestamp,
  resolveAvailableNodes,
  resolveNodeRoleLabel,
} from "./agent_node_detail_shared";

type AgentNodeDraft = {
  name: string;
  grpcTarget: string;
  tlsServerName: string;
  defaultWorktreeRoot: string;
};

type BootstrapJoinContentProps = {
  nodeJoinBootstrap: AgentNodeJoinBootstrapInfo | null;
  nodeJoinBootstrapLoading: boolean;
  nodeJoinBootstrapError: string | null;
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
  onOpenNodeDetail?: (nodeId: string) => void;
};

function toNodeDraft(node: AgentNodeRecord): AgentNodeDraft {
  return {
    name: node.name,
    grpcTarget: node.grpc_target ?? "",
    tlsServerName: node.tls_server_name ?? "",
    defaultWorktreeRoot: node.default_worktree_root ?? "",
  };
}

const MACHINE_ROSTER_ITEM_CLASS =
  "w-full rounded-xl border-2 border-transparent bg-white/70 px-2.5 py-2.5 shadow-none hover:border-black hover:bg-white";
const MACHINE_ROSTER_ITEM_ACTIVE_CLASS = "border-black bg-white";

function renderBootstrapJoinContent({
  nodeJoinBootstrap,
  nodeJoinBootstrapLoading,
  nodeJoinBootstrapError,
}: BootstrapJoinContentProps): React.ReactNode {
  if (nodeJoinBootstrapLoading) {
    return (
      <Text size="xs" c="dimmed">
        Loading node bootstrap details...
      </Text>
    );
  }

  if (nodeJoinBootstrapError) {
    return (
      <Alert color="red" variant="light" title="Bootstrap unavailable">
        <Text size="sm">{nodeJoinBootstrapError}</Text>
      </Alert>
    );
  }

  if (!nodeJoinBootstrap) {
    return (
      <Text size="xs" c="dimmed">
        Agent Node Join Bootstrap details are not available yet.
      </Text>
    );
  }

  if (!nodeJoinBootstrap.enabled) {
    return (
      <Alert color="yellow" variant="light" title="Internal gRPC required">
        <Text size="sm">
          Enable
          {" "}
          <code>[internal_grpc]</code>
          {" "}
          on the main control plane before joining remote nodes by token.
        </Text>
      </Alert>
    );
  }

  return (
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
        <TextInput label="Join token" value={nodeJoinBootstrap.bootstrap_token ?? ""} readOnly />
        <TextInput
          label="Control-plane gRPC listen"
          value={nodeJoinBootstrap.grpc_listen_addr ?? ""}
          readOnly
        />
      </Group>
      <Group grow align="end">
        <TextInput label="Security mode" value={nodeJoinBootstrap.security_mode ?? ""} readOnly />
        <TextInput label="Issuer" value={nodeJoinBootstrap.issuer ?? ""} readOnly />
        <TextInput label="Audience" value={nodeJoinBootstrap.audience ?? ""} readOnly />
      </Group>
      <TextInput label="Cert directory" value={nodeJoinBootstrap.cert_dir ?? ""} readOnly />
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
  );
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
  onOpenNodeDetail,
}: AgentNodeSectionProps) {
  const createNodeError = validateAgentNodeDraft({
    nodeId: nodeIdInput,
    nodeName: nodeNameInput,
    grpcTarget: grpcTargetInput,
  });
  const availableNodes = React.useMemo(() => resolveAvailableNodes(nodes), [nodes]);
  const resolvedTargetNodeId = targetNodeId.trim() || "main";
  const selectedNode =
    availableNodes.find((node) => node.id === resolvedTargetNodeId) ??
    availableNodes.find((node) => node.is_main) ??
    null;
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
  const selectedNodeAgents = React.useMemo(
    () => (selectedNode ? agentsByNodeId.get(selectedNode.id) ?? [] : []),
    [agentsByNodeId, selectedNode]
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
            <Text fw={600}>Connect machine</Text>
            <Text size="xs" c="dimmed">
              Use the bootstrap token on the remote machine, then register the reachable gRPC route
              once it can talk to this control plane.
            </Text>
          </div>
          {renderBootstrapJoinContent({
            nodeJoinBootstrap,
            nodeJoinBootstrapLoading,
            nodeJoinBootstrapError,
          })}
        </Stack>
      </div>

      <div className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-4 shadow-sm">
        <Stack gap="sm">
          <div>
            <Text fw={600}>Machines</Text>
            <Text size="xs" c="dimmed">
              Select a machine from the roster, then inspect its details and attached agents in one
              place.
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
                  className={[
                    MACHINE_ROSTER_ITEM_CLASS,
                    isSelected ? MACHINE_ROSTER_ITEM_ACTIVE_CLASS : "",
                  ].join(" ")}
                  onClick={() => onTargetNodeIdChange(node.id)}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <Text size="sm" fw={600}>
                          {node.name}
                        </Text>
                        <Badge tone={node.is_main ? "subtle" : "outline"} className="uppercase">
                          {resolveNodeRoleLabel(node)}
                        </Badge>
                        <Text size="xs" c="dimmed">
                          {nodeAgents.length} agent{nodeAgents.length === 1 ? "" : "s"}
                        </Text>
                      </div>
                      <Text size="xs" c="dimmed" mt={2}>
                        {node.is_main
                          ? "Run on this AgentHub instance."
                          : node.grpc_target ?? "Encrypted gRPC target"}
                      </Text>
                      {node.default_worktree_root ? (
                        <Text size="xs" c="dimmed" mt={2}>
                          Worktree root: {node.default_worktree_root}
                        </Text>
                      ) : null}
                    </div>
                    {isSelected ? (
                      <Badge tone="outline" className="shrink-0">
                        Selected
                      </Badge>
                    ) : null}
                  </div>
                  {nodeAgents.length > 0 ? (
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      {nodeAgents.slice(0, 4).map((agent) => (
                        <span
                          key={agent.id}
                          className="inline-flex items-center rounded-full border border-ui-border/80 bg-white px-2 py-0.5 text-[11px] text-ui-text-secondary"
                        >
                          {agent.name}
                        </span>
                      ))}
                      {nodeAgents.length > 4 ? (
                        <span className="inline-flex items-center rounded-full border border-ui-border/80 bg-white px-2 py-0.5 text-[11px] text-ui-text-secondary">
                          +{nodeAgents.length - 4} more
                        </span>
                      ) : null}
                    </div>
                  ) : (
                    <Text size="xs" c="dimmed" mt={8}>
                      No agents assigned yet.
                    </Text>
                  )}
                </SelectableListItem>
              );
            })}
          </div>
          {selectedNode ? (
            <Stack gap="sm">
              <div className="flex items-center justify-end">
                {onOpenNodeDetail ? (
                  <Button
                    variant="light"
                    size="xs"
                    onClick={() => onOpenNodeDetail(selectedNode.id)}
                  >
                    Open node detail
                  </Button>
                ) : null}
              </div>
              <AgentNodeDetailCard
                node={selectedNode}
                agents={selectedNodeAgents}
                nodeJoinBootstrap={nodeJoinBootstrap}
                nodeJoinBootstrapLoading={nodeJoinBootstrapLoading}
                nodeJoinBootstrapError={nodeJoinBootstrapError}
                compact
              />
            </Stack>
          ) : null}
        </Stack>
      </div>

      <div className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-4 shadow-sm">
        <Stack gap="sm">
          <div>
            <Text fw={600}>Add machine</Text>
            <Text size="xs" c={createNodeError ? "red" : "dimmed"}>
              {createNodeError ??
                "Register a remote machine once, then keep routing agents from the roster above."}
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
              Machine settings
            </Text>
            {remoteNodes.length === 0 ? (
              <Text size="xs" c="dimmed">
                No remote machines added yet.
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
                    className="rounded-xl border border-ui-border bg-white/92 px-3 py-3 shadow-sm"
                  >
                    <Stack gap="sm">
                      <Group justify="space-between" align="flex-start">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <Text size="sm" fw={600}>
                              {node.name}
                            </Text>
                            <Badge tone="outline" className="uppercase">
                              remote
                            </Badge>
                            <Text size="xs" c="dimmed">
                              {node.id}
                            </Text>
                          </div>
                          <Text size="xs" c="dimmed">
                            Update routing and default worktree behavior for this machine.
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
