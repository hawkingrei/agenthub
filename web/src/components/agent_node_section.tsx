import { Alert, Button, Group, Select, Stack, Text, TextInput } from "@mantine/core";
import { AgentNodeRecord } from "../api";

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
  createBusy: boolean;
  deletingNodeIds: Record<string, boolean>;
  onCreateNode: () => void;
  onDeleteNode: (nodeId: string) => void;
};

export function validateAgentNodeDraft(input: {
  nodeId: string;
  nodeName: string;
  grpcTarget: string;
}): string | null {
  if (!input.nodeId.trim()) {
    return "Node ID is required.";
  }
  if (!input.nodeName.trim()) {
    return "Node name is required.";
  }
  if (!input.grpcTarget.trim()) {
    return "gRPC target is required.";
  }
  return null;
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
  createBusy,
  deletingNodeIds,
  onCreateNode,
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
              remoteNodes.map((node) => (
                <div
                  key={node.id}
                  className="flex items-center justify-between gap-3 rounded-xl border border-ui-border bg-white px-3 py-2"
                >
                  <div className="min-w-0">
                    <Text size="sm" fw={600}>
                      {node.name}
                    </Text>
                    <Text size="xs" c="dimmed" truncate="end">
                      {node.grpc_target ?? "gRPC target pending"}
                    </Text>
                  </div>
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
                </div>
              ))
            )}
          </Stack>
        </Stack>
      </div>
    </Stack>
  );
}
