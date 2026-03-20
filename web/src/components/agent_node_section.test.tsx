import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";

import type { AgentNodeRecord } from "../api";
import {
  AgentNodeSection,
  validateAgentNodeDraft,
  validateAgentNodeUpdateDraft,
} from "./agent_node_section";

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
      nodeNameInput: "Node East",
      grpcTargetInput: "https://node-east.internal:50051",
    });
    expect(html).toContain("Node ID is required.");
    expect(html).toContain("Add Node");
    expect(html).toContain("disabled");
  });

  it("keeps the default helper copy when the draft is complete", () => {
    const html = renderSection({
      nodeIdInput: "node-east",
      nodeNameInput: "Node East",
      grpcTargetInput: "https://node-east.internal:50051",
    });
    expect(html).toContain("AgentHub to node and node to node traffic uses encrypted gRPC.");
    expect(html).toContain("Add Node");
    expect(html).not.toContain("Node ID is required.");
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
});
