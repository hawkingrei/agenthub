import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";

import { AgentNodesWorkbench } from "./agent_nodes_workbench";

const baseProps: ComponentProps<typeof AgentNodesWorkbench> = {
  nodes: [
    {
      id: "main",
      name: "Main Node",
      grpc_target: null,
      tls_server_name: null,
      default_worktree_root: null,
      last_seen_at: null,
      is_main: true,
      created_at: 0,
      updated_at: 0,
    },
    {
      id: "node-east",
      name: "Node East",
      grpc_target: "https://node-east.internal:50051",
      tls_server_name: "node-east.internal",
      default_worktree_root: "~/.agenthub/worktrees/node-east",
      last_seen_at: null,
      is_main: false,
      created_at: 1,
      updated_at: 1,
    },
  ],
  agents: [],
  teams: [],
  selectedNodeId: "node-east",
  nodeJoinBootstrap: {
    enabled: true,
    bootstrap_token: "bootstrap-token",
  },
  nodeJoinBootstrapLoading: false,
  nodeJoinBootstrapError: null,
  updatingNodeIds: {},
  deletingNodeIds: {},
  onSelectNode: () => {},
  onOpenAgent: () => {},
  onCreateAgent: () => {},
  onUpdateNode: () => {},
  onDeleteNode: () => {},
};

const renderWorkbench = (overrides?: Partial<ComponentProps<typeof AgentNodesWorkbench>>) =>
  renderToStaticMarkup(
    <MantineProvider>
      <AgentNodesWorkbench {...baseProps} {...overrides} />
    </MantineProvider>
  );

describe("AgentNodesWorkbench", () => {
  it("renders settings and danger zone on remote node detail", () => {
    const html = renderWorkbench({
      agents: [
        {
          id: "agent-remote-1",
          name: "Worker A",
          command: "agenthub",
          args: [],
          workdir: "/tmp/worker-a",
          status: "running",
          target_node_id: "node-east",
          worktree_mode: "use_existing",
          code_mode: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
      teams: [
        {
          id: "team-1",
          name: "Papers We Love",
          description: null,
          spec: {
            leader_member_id: "agent-remote-1",
            members: [
              { member_id: "agent-remote-1", role: "leader" },
              { member_id: "worker-2", role: "worker" },
            ],
          },
          created_at: 1,
          updated_at: 1,
        },
      ],
    });

    expect(html).toContain("Node Detail");
    expect(html).toContain("Connect");
    expect(html).toContain("Connect Command");
    expect(html).toContain("Connect Config");
    expect(html).toContain("Agent Activity Detected");
    expect(html).toContain("indirect runtime signal");
    expect(html).toContain("bootstrap-token");
    expect(html).toContain("Copy");
    expect(html).toContain("node_id=node-east");
    expect(html).toContain("needs: config path");
    expect(html).toContain("server.role");
    expect(html).toContain("internal_grpc.bootstrap.token");
    expect(html).toContain("Runtime signal");
    expect(html).toContain("Registry evidence");
    expect(html).toContain("Settings");
    expect(html).toContain("Save Settings");
    expect(html).toContain("Teams Using This Node");
    expect(html).toContain("Papers We Love");
    expect(html).toContain("1 team");
    expect(html).toContain("1 members");
    expect(html).toContain("1 active");
    expect(html).toContain("1 leaders");
    expect(html).toContain("0 workers");
    expect(html).toContain("1 agent · 1 team");
    expect(html).toContain("Worker A · leader");
    expect(html).toContain("Danger Zone");
    expect(html).toContain("This node still has 1 attached agent.");
    expect(html).toContain("Delete Node");
    expect(html).toContain("disabled");
  });

  it("keeps the local main node danger zone read only", () => {
    const html = renderWorkbench({
      selectedNodeId: "main",
    });

    expect(html).toContain("Main Node");
    expect(html).toContain("Connected");
    expect(html).toContain("Danger Zone");
    expect(html).toContain("cannot be deleted");
    expect(html).not.toContain("Save Settings");
  });

  it("keeps an explicit placeholder when bootstrap token data is unavailable", () => {
    const html = renderWorkbench({
      nodeJoinBootstrap: {
        enabled: true,
      },
    });

    expect(html).toContain("&lt;bootstrap-token-from-main-control-plane&gt;");
    expect(html).toContain("needs: bootstrap token");
    expect(html).toContain("explicit token placeholder");
    expect(html).toContain("Unverified");
  });

  it("prefers persisted last_seen_at over indirect agent activity hints", () => {
    const now = Math.floor(Date.now() / 1000);
    const html = renderWorkbench({
      nodes: baseProps.nodes.map((node) =>
        node.id === "node-east" ? { ...node, last_seen_at: now } : node
      ),
      agents: [
        {
          id: "agent-remote-1",
          name: "Worker A",
          command: "agenthub",
          args: [],
          workdir: "/tmp/worker-a",
          status: "running",
          target_node_id: "node-east",
          worktree_mode: "use_existing",
          code_mode: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
    });

    expect(html).toContain("Recently Seen");
    expect(html).toContain("Last seen");
    expect(html).toContain("lightweight node last-seen signal");
    expect(html).not.toContain("Agent Activity Detected");
  });
});
