import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";

import {
  AgentNodeSection,
  validateAgentNodeDraft,
} from "./agent_node_section";

const baseProps = {
  nodes: [
    {
      id: "main",
      name: "Main Node",
      grpc_target: null,
      tls_server_name: null,
      is_main: true,
      created_at: 0,
      updated_at: 0,
    },
  ],
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
  createBusy: false,
  deletingNodeIds: {},
  onCreateNode: () => {},
  onDeleteNode: () => {},
};

const renderSection = (overrides?: Partial<typeof baseProps>) =>
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
        nodeId: "node-east",
        nodeName: "Node East",
        grpcTarget: "https://node-east.internal:50051",
      })
    ).toBeNull();
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
});
