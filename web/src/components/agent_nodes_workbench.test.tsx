// @vitest-environment jsdom
import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";

import {
  AgentNodesWorkbench,
  buildNodeEditDraft,
  buildNodeNameUpdatePayload,
  buildNodeSettingsUpdatePayload,
} from "./agent_nodes_workbench";
import {
  installReactDomTestGlobals,
  renderWithMantine,
  required,
} from "../test_utils/react_test_helpers";

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

function findButtonByText(container: HTMLElement, text: string): HTMLButtonElement {
  return required(
    Array.from(container.querySelectorAll("button")).find((node) =>
      node.textContent?.includes(text)
    ) as HTMLButtonElement | undefined,
    `${text} button missing`
  );
}

function changeInputValue(input: HTMLInputElement, value: string): void {
  const descriptor = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(input) as HTMLInputElement,
    "value"
  );
  descriptor?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

describe("AgentNodesWorkbench", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    installReactDomTestGlobals();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("renders name, settings, and danger zone on remote node detail", () => {
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
            coordinator_member_id: "agent-remote-1",
            members: [
              { member_id: "agent-remote-1", role: "coordinator" },
              { member_id: "worker-2", role: "worker" },
            ],
          },
          created_at: 1,
          updated_at: 1,
        },
      ],
    });

    expect(html).toContain("Node Detail");
    expect(html).toContain("Connect Command");
    expect(html).toContain("Connect Config");
    expect(html).toContain("Degraded");
    expect(html).toContain("indirect runtime signal");
    expect(html).toContain("Observed Agent Runtimes");
    expect(html).toContain("AgentHub Runtime");
    expect(html).toContain("Codex CLI (no attached agent observed)");
    expect(html).toContain("Gemini CLI (no attached agent observed)");
    expect(html).toContain("bootstrap-token");
    expect(html).toContain("Copy");
    expect(html).toContain("node_id=node-east");
    expect(html).toContain("needs: config path");
    expect(html).toContain("server.role");
    expect(html).toContain("internal_grpc.bootstrap.token");
    expect(html).toContain("Runtime signal");
    expect(html).toContain("Registry evidence");
    expect(html).toContain("Name");
    expect(html).toContain("Save Name");
    expect(html).toContain("Settings");
    expect(html).toContain("Save Settings");
    expect(html).toContain("Teams Using This Node");
    expect(html).toContain("Papers We Love");
    expect(html).toContain('href="/workspace/teams/team-1"');
    expect(html).toContain("1 team");
    expect(html).toContain("1 member");
    expect(html).toContain("1 active");
    expect(html).toContain("Coordinators");
    expect(html).toContain("Workers");
    expect(html).toContain("Member Runtime Drill-down");
    expect(html).toContain("1 agent · 1 team");
    expect(html).toContain("Worker A · coordinator");
    expect(html).toContain("Working");
    expect(html).toContain("AgentHub Runtime");
    expect(html).toContain("Worktree: Existing workdir");
    expect(html).toContain('href="/workspace/teams/team-1/members/agent-remote-1/thread"');
    expect(
      html
    ).toContain('href="/workspace/teams/team-1/members/agent-remote-1/member_console"');
    expect(html).toContain("Thread");
    expect(html).toContain("Console");
    expect(html).toContain("Danger Zone");
    expect(html).toContain("This node still has 1 attached agent.");
    expect(html).toContain("Delete Node");
    expect(html).toContain("disabled");
    expect(html).toContain("lg:grid-cols-[240px_minmax(0,1fr)]");
    expect(html).toContain("min-[420px]:grid-cols-2 sm:grid-cols-4");
    expect(html).toContain("min-w-0 w-full gap-2 min-[420px]:grid-cols-2");
  });

  it("keeps the local main node danger zone read only", () => {
    const html = renderWorkbench({
      selectedNodeId: "main",
    });

    expect(html).toContain("Main Node");
    expect(html).toContain("Connected");
    expect(html).toContain("Name");
    expect(html).toContain("Danger Zone");
    expect(html).toContain("read only");
    expect(html).toContain("cannot be deleted");
    expect(html).not.toContain("Save Name");
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
    expect(html).toContain("Offline");
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

    expect(html).toContain("Connected");
    expect(html).toContain("Last seen");
    expect(html).toContain("lightweight node last-seen signal");
    expect(html).not.toContain("indirect runtime signal");
  });

  it("uses fallback member agents when the root agents list hides team members", () => {
    const html = renderWorkbench({
      agents: [],
      teams: [
        {
          id: "team-1",
          name: "tidb fuzz/bugfix team",
          description: null,
          spec: {
            members: [{ member_id: "hidden-worker", role: "worker" }],
          },
          created_at: 1,
          updated_at: 1,
        },
      ],
      teamMemberAgentsById: {
        "hidden-worker": {
          id: "hidden-worker",
          name: "tidb-fuzz-bugfix-team-worker-1",
          command: "agenthub",
          args: [],
          workdir: "/tmp/hidden-worker",
          status: "idle",
          target_node_id: "node-east",
          worktree_mode: "use_existing",
          code_mode: false,
          created_at: 1,
          updated_at: 1,
        },
      },
    });

    expect(html).toContain("tidb fuzz/bugfix team");
    expect(html).toContain("1 team");
    expect(html).toContain("1 member");
    expect(html).toContain("tidb-fuzz-bugfix-team-worker-1 · worker");
  });

  it("shows explicit runtime/provider badges for attached agents", () => {
    const html = renderWorkbench({
      agents: [
        {
          id: "agent-gemini-1",
          name: "Gemini Worker",
          command: "gemini --model gemini-pro",
          args: [],
          workdir: "/tmp/gemini-worker",
          status: "idle",
          target_node_id: "node-east",
          worktree_mode: "create_worktree",
          code_mode: false,
          created_at: 1,
          updated_at: 1,
        },
        {
          id: "agent-custom-1",
          name: "Custom Worker",
          command: "python worker.py",
          args: [],
          workdir: "/tmp/custom-worker",
          status: "stopped",
          target_node_id: "node-east",
          worktree_mode: "reuse_worktree",
          code_mode: false,
          created_at: 1,
          updated_at: 1,
        },
      ],
    });

    expect(html).toContain("Gemini Worker");
    expect(html).toContain("Gemini CLI");
    expect(html).toContain("Custom Worker");
    expect(html).toContain("Custom Runtime");
  });

  it("builds a name-update payload without dropping persisted routing metadata", () => {
    expect(buildNodeEditDraft(baseProps.nodes[1])).toEqual({
      name: "Node East",
      grpcTarget: "https://node-east.internal:50051",
      tlsServerName: "node-east.internal",
      defaultWorktreeRoot: "~/.agenthub/worktrees/node-east",
    });

    expect(
      buildNodeNameUpdatePayload(baseProps.nodes[1], {
        ...buildNodeEditDraft(baseProps.nodes[1]),
        name: "Node East Renamed",
      })
    ).toEqual({
      name: "Node East Renamed",
      grpc_target: "https://node-east.internal:50051",
      tls_server_name: "node-east.internal",
      default_worktree_root: "~/.agenthub/worktrees/node-east",
    });
  });

  it("keeps unsaved routing edits out of the name-only payload", () => {
    expect(
      buildNodeNameUpdatePayload(baseProps.nodes[1], {
        name: "Node East Renamed",
        grpcTarget: "https://unsaved-change.internal:60061",
        tlsServerName: "unsaved-change.internal",
        defaultWorktreeRoot: "/srv/unsaved-change",
      })
    ).toEqual({
      name: "Node East Renamed",
      grpc_target: "https://node-east.internal:50051",
      tls_server_name: "node-east.internal",
      default_worktree_root: "~/.agenthub/worktrees/node-east",
    });
  });

  it("builds a settings-update payload without mutating the persisted node name", () => {
    expect(
      buildNodeSettingsUpdatePayload(baseProps.nodes[1], {
        grpcTarget: "https://node-east.internal:60061",
        tlsServerName: "node-east-alt.internal",
        defaultWorktreeRoot: "/srv/agenthub/worktrees/node-east",
      })
    ).toEqual({
      name: "Node East",
      grpc_target: "https://node-east.internal:60061",
      tls_server_name: "node-east-alt.internal",
      default_worktree_root: "/srv/agenthub/worktrees/node-east",
    });
  });

  it("refuses to build a name-update payload when no routing target exists", () => {
    expect(
      buildNodeNameUpdatePayload(
        {
          ...baseProps.nodes[1],
          grpc_target: null,
        },
        {
          name: "Node East Renamed",
          grpcTarget: "",
          tlsServerName: "",
          defaultWorktreeRoot: "",
        }
      )
    ).toBeNull();
  });

  it("disables remote node name saves until the required routing metadata exists", () => {
    const html = renderWorkbench({
      nodes: baseProps.nodes.map((node) =>
        node.id === "node-east" ? { ...node, grpc_target: null } : node
      ),
    });

    expect(html).toContain("This node is missing a persisted gRPC target.");
  });

  it("renders the no-team empty state when no teams use the selected node", () => {
    const html = renderWorkbench({
      agents: [],
      teams: [],
    });

    expect(html).toContain("Teams Using This Node");
    expect(html).toContain("No team attachments yet");
    expect(html).toContain("No current team members resolve to this node");
  });

  it("routes node roster, create-agent, open-agent, and delete-node actions", () => {
    const onSelectNode = vi.fn();
    const onCreateAgent = vi.fn();
    const onOpenAgent = vi.fn();
    const onDeleteNode = vi.fn();

    renderWithMantine(
      root,
      <AgentNodesWorkbench
        {...baseProps}
        agents={[
          {
            id: "agent-remote-1",
            name: "Worker A",
            command: "agenthub",
            args: [],
            workdir: "/tmp/worker-a",
            status: "idle",
            target_node_id: "node-east",
            worktree_mode: "use_existing",
            code_mode: false,
            created_at: 1,
            updated_at: 1,
          },
        ]}
        onSelectNode={onSelectNode}
        onCreateAgent={onCreateAgent}
        onOpenAgent={onOpenAgent}
        onDeleteNode={onDeleteNode}
        deletingNodeIds={{ "node-east": false }}
      />
    );

    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((node) =>
          node.textContent?.includes("Main Node")
        ) as HTMLButtonElement | undefined,
        "main node roster button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((node) =>
          node.textContent?.includes("Create Agent")
        ) as HTMLButtonElement | undefined,
        "create agent button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((node) =>
          node.textContent?.includes("Open")
        ) as HTMLButtonElement | undefined,
        "open agent button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(onSelectNode).toHaveBeenCalledWith("main");
    expect(onCreateAgent).toHaveBeenCalledTimes(1);
    expect(onOpenAgent).toHaveBeenCalledWith("agent-remote-1");

    renderWithMantine(
      root,
      <AgentNodesWorkbench
        {...baseProps}
        agents={[]}
        onDeleteNode={onDeleteNode}
      />
    );

    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((node) =>
          node.textContent?.includes("Delete Node")
        ) as HTMLButtonElement | undefined,
        "delete node button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(onDeleteNode).toHaveBeenCalledWith("node-east");
  });

  it("routes remote node editor saves and drill-down links", () => {
    const onUpdateNode = vi.fn();
    renderWithMantine(
      root,
      <AgentNodesWorkbench
        {...baseProps}
        teams={[
          {
            id: "team-1",
            name: "Team One",
            description: null,
            spec: {
              members: [{ member_id: "agent-remote-1", role: "coordinator" }],
            },
            created_at: 1,
            updated_at: 1,
          },
        ]}
        agents={[
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
        ]}
        onUpdateNode={onUpdateNode}
      />
    );

    const inputs = Array.from(container.querySelectorAll("input"));
    const [nameInput, grpcTargetInput, tlsServerNameInput, worktreeRootInput] = inputs;
    expect(nameInput).toBeDefined();
    expect(grpcTargetInput).toBeDefined();
    expect(tlsServerNameInput).toBeDefined();
    expect(worktreeRootInput).toBeDefined();

    act(() => {
      changeInputValue(nameInput, "Node East Renamed");
    });
    act(() => {
      findButtonByText(container, "Save Name").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });

    act(() => {
      changeInputValue(grpcTargetInput, "https://node-east.internal:60061");
      changeInputValue(tlsServerNameInput, "node-east-alt.internal");
      changeInputValue(worktreeRootInput, "/srv/agenthub/worktrees/node-east");
    });
    act(() => {
      findButtonByText(container, "Save Settings").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });

    expect(onUpdateNode).toHaveBeenNthCalledWith(1, "node-east", {
      name: "Node East Renamed",
      grpc_target: "https://node-east.internal:50051",
      tls_server_name: "node-east.internal",
      default_worktree_root: "~/.agenthub/worktrees/node-east",
    });
    expect(onUpdateNode).toHaveBeenNthCalledWith(2, "node-east", {
      name: "Node East",
      grpc_target: "https://node-east.internal:60061",
      tls_server_name: "node-east-alt.internal",
      default_worktree_root: "/srv/agenthub/worktrees/node-east",
    });

    const links = Array.from(container.querySelectorAll("a")) as HTMLAnchorElement[];
    const teamLink = links.find((link) => link.textContent?.includes("Team One"));
    const threadLink = links.find((link) => link.textContent?.trim() === "Thread");
    const consoleLink = links.find((link) => link.textContent?.trim() === "Console");
    expect(teamLink).toBeDefined();
    expect(threadLink).toBeDefined();
    expect(consoleLink).toBeDefined();

    act(() => {
      teamLink?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(window.location.pathname).toBe("/workspace/teams/team-1");

    act(() => {
      threadLink?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(window.location.pathname).toBe("/workspace/teams/team-1/members/agent-remote-1/thread");

    act(() => {
      consoleLink?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(window.location.pathname).toBe(
      "/workspace/teams/team-1/members/agent-remote-1/member_console"
    );
  });
});
