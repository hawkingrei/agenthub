import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";

import type { AgentNodeRecord } from "../api";
import { AgentNodeSection } from "./agent_node_section";
import { validateAgentNodeDraft, validateAgentNodeUpdateDraft } from "./agent_node_validation";

const baseNodes: AgentNodeRecord[] = [
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

const baseProps: ComponentProps<typeof AgentNodeSection> = {
  nodes: baseNodes,
  agents: [],
  nodeJoinBootstrap: null,
  nodeJoinBootstrapLoading: false,
  nodeJoinBootstrapError: null,
  targetNodeId: "main",
  onTargetNodeIdChange: () => {},
  nodeIdInput: "",
  onNodeIdInputChange: () => {},
  nodeNameInput: "",
  onNodeNameInputChange: () => {},
  grpcTargetInput: "",
  onGrpcTargetInputChange: () => {},
  tlsServerNameInput: "",
  onTlsServerNameInputChange: () => {},
  defaultWorktreeRootInput: "",
  onDefaultWorktreeRootInputChange: () => {},
  createBusy: false,
  updatingNodeIds: {},
  deletingNodeIds: {},
  onCreateNode: () => {},
  onUpdateNode: () => {},
  onDeleteNode: () => {},
};

const renderSection = (overrides?: Partial<ComponentProps<typeof AgentNodeSection>>) =>
  renderToStaticMarkup(
    <MantineProvider>
      <AgentNodeSection {...baseProps} {...overrides} />
    </MantineProvider>
  );

describe("AgentNodeSection", () => {
  it("validates required draft fields before enabling create", () => {
    expect(
      validateAgentNodeDraft({
        nodeId: "",
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node ID is required.");
    expect(
      validateAgentNodeDraft({
        nodeId: "node-east",
        nodeName: "",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node name is required.");
    expect(
      validateAgentNodeDraft({
        nodeId: "node-east",
        nodeName: "Node East",
        grpcTarget: "",
      })
    ).toBe("gRPC target is required.");
    expect(
      validateAgentNodeDraft({
        nodeId: "main",
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node ID 'main' is reserved.");
    expect(
      validateAgentNodeDraft({
        nodeId: "node east",
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node ID may only contain ASCII letters, numbers, '.', '_', '-', or ':'.");
    expect(
      validateAgentNodeDraft({
        nodeId: "node-east",
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBeNull();
    expect(
      validateAgentNodeDraft({
        nodeId: "x".repeat(129),
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node ID must be at most 128 characters.");
    expect(
      validateAgentNodeUpdateDraft({
        nodeName: "",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBe("Node name is required.");
    expect(
      validateAgentNodeUpdateDraft({
        nodeName: "Node East",
        grpcTarget: "",
      })
    ).toBe("gRPC target is required.");
  });

  it("renders inline validation guidance when the draft is incomplete", () => {
    const html = renderSection({
      nodeJoinBootstrapLoading: true,
      nodeNameInput: "Node East",
      grpcTargetInput: "https://node-east.internal:50051",
    });
    expect(html).toContain("Node ID is required.");
    expect(html).toContain("Add Node");
    expect(html).toContain("disabled");
    expect(html).toContain("Loading node bootstrap details...");
  });

  it("keeps the default helper copy when the draft is complete", () => {
    const html = renderSection({
      nodeJoinBootstrap: {
        enabled: true,
        bootstrap_token: "bootstrap-token",
        grpc_listen_addr: "0.0.0.0:50051",
        security_mode: "tls",
        cert_dir: "/etc/agenthub/internal-grpc",
        issuer: "agenthub",
        audience: "agenthub-internal",
      },
      nodeIdInput: "node-east",
      nodeNameInput: "Node East",
      grpcTargetInput: "https://node-east.internal:50051",
    });
    expect(html).toContain("Join node with token");
    expect(html).toContain("bootstrap-token");
    expect(html).toContain("Control-plane gRPC listen");
    expect(html).toContain("0.0.0.0:50051");
    expect(html).toContain("AgentHub to node and node to node traffic uses encrypted gRPC.");
    expect(html).toContain("Add Node");
    expect(html).not.toContain("Node ID is required.");
  });

  it("renders a disabled bootstrap hint when internal gRPC is unavailable", () => {
    const html = renderSection({
      nodeJoinBootstrap: {
        enabled: false,
      },
    });
    expect(html).toContain("Internal gRPC required");
    expect(html).toContain("before joining remote nodes by token");
  });

  it("renders a named bootstrap error state when loading fails", () => {
    const html = renderSection({
      nodeJoinBootstrapError: "Agent Node Join Bootstrap: Error: boom",
    });
    expect(html).toContain("Bootstrap unavailable");
    expect(html).toContain("Agent Node Join Bootstrap: Error: boom");
  });

  it("renders selected node default worktree root guidance", () => {
    const html = renderSection({
      nodes: [
        baseProps.nodes[0],
        {
          id: "node-east",
          name: "Node East",
          grpc_target: "https://node-east.internal:50051",
          tls_server_name: "node-east.internal",
          default_worktree_root: "~/.agenthub/worktrees/node-east",
          is_main: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
      targetNodeId: "node-east",
    });
    expect(html).toContain("Blank create-worktree workdirs default to:");
    expect(html).toContain("~/.agenthub/worktrees/node-east");
    expect(html).toContain("Save");
  });

  it("marks the selected node chooser button with aria-pressed", () => {
    const html = renderSection({
      nodes: [
        baseProps.nodes[0],
        {
          id: "node-east",
          name: "Node East",
          grpc_target: "https://node-east.internal:50051",
          tls_server_name: "node-east.internal",
          default_worktree_root: "~/.agenthub/worktrees/node-east",
          is_main: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
      targetNodeId: "node-east",
    });

    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain('aria-pressed="false"');
  });

  it("renders machine summaries and attached agents for local and remote nodes", () => {
    const html = renderSection({
      nodes: [
        baseProps.nodes[0],
        {
          id: "node-east",
          name: "Node East",
          grpc_target: "https://node-east.internal:50051",
          tls_server_name: "node-east.internal",
          default_worktree_root: "~/.agenthub/worktrees/node-east",
          is_main: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
      agents: [
        {
          id: "agent-main-1",
          name: "Planner",
          workdir: "/tmp/planner",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: null,
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-remote-1",
          name: "Worker A",
          workdir: "/tmp/worker-a",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "node-east",
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-remote-2",
          name: "Worker B",
          workdir: "/tmp/worker-b",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "node-east",
          created_at: 1,
          updated_at: 1,
        },
      ] as typeof baseProps.agents,
      targetNodeId: "node-east",
    });

    expect(html).toContain("Machines &amp; Agents");
    expect(html).toContain("Main Node");
    expect(html).toContain("Node East");
    expect(html).toContain("Planner");
    expect(html).toContain("Worker A");
    expect(html).toContain("Worker B");
    expect(html).toContain("Selected");
    expect(html).toContain("Bind the agent to Node East via encrypted gRPC (node-east.internal).");
  });

  it("falls back to the main node chooser and truncates long agent lists", () => {
    const html = renderSection({
      nodes: [],
      agents: [
        {
          id: "agent-1",
          name: "One",
          workdir: "/tmp/one",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "main",
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-2",
          name: "Two",
          workdir: "/tmp/two",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "main",
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-3",
          name: "Three",
          workdir: "/tmp/three",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "main",
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-4",
          name: "Four",
          workdir: "/tmp/four",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "main",
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-5",
          name: "Five",
          workdir: "/tmp/five",
          status: "running",
          mode: "normal",
          source: null,
          target_node_id: "main",
          created_at: 1,
          updated_at: 1,
        },
      ] as typeof baseProps.agents,
    });

    expect(html).toContain("Run on this AgentHub instance.");
    expect(html).toContain("+1 more");
    expect(html).not.toContain("No agents assigned yet.");
  });
});
